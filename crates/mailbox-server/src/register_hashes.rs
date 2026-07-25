use axum::{body::Bytes, extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::{
    blob_refs_table::OpRef,
    scrub_blobs::{blob_is_wanted, record_blob_refs},
    AppState, BlobSync,
};

/// A client's request to announce blob hashes to the mailbox. For each hash the
/// mailbox already holds it reports `already_stored`; every other hash it
/// registers in its fetch pool so it pulls the blob from `sender_pubkey`. Blob
/// bytes are never carried here — clients push those separately to
/// `/blobs/upload` and use this announce to reconcile what actually landed.
#[derive(Serialize, Deserialize)]
pub struct RegisterHashesRequest {
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    /// Opaque identifier of the operation these blobs belong to. The mailbox
    /// records a `(blob, operation)` reference for each hash, which is what
    /// `/blobs/scrub` later tombstones — blobs are content-addressed over
    /// plaintext media, so the bare hash is not a safe unit of deletion. Left
    /// empty for a re-announce of blobs already vouched for by an earlier
    /// request, which refreshes the mailbox's fetch intent without minting a
    /// new reference.
    #[serde(default)]
    pub op_ref: OpRef,
    /// The peer the mailbox should dial to fetch any hash it does not already
    /// hold (typically the sending client itself).
    pub sender_pubkey: iroh::EndpointId,
    /// Set when the client intends to stream the announced blobs to
    /// `/blobs/upload` right after this announce. The mailbox then defers dialing
    /// `sender_pubkey` by its fixed grace window so the upload can land first
    /// without a duplicate transfer. Clients that only announce (no bytes to
    /// push, e.g. the re-announce followup) leave it `false` for an immediate
    /// fetch. `#[serde(default)]` keeps older payloads that omit it decodable.
    #[serde(default)]
    pub expect_upload: bool,
    /// Reserved for a `sender_pubkey` signature over the request; currently
    /// unverified, so clients send it empty. `#[serde(default)]` keeps older
    /// payloads that omit the field entirely decodable.
    #[serde(default)]
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterHashesResponse {
    /// Hashes the mailbox already has stored (empty if it has stored none).
    pub already_stored: Vec<iroh_blobs::Hash>,
}

#[derive(Serialize, Deserialize)]
pub struct UploadBlobResponse {
    /// The hash the uploaded bytes hash to, so the client can confirm the
    /// mailbox stored what it intended.
    pub hash: iroh_blobs::Hash,
}

/// Register `source` as a provider for each hash the mailbox does not yet hold.
/// When `expect_upload` is set, each fetch is deferred by the mailbox's fixed
/// grace window so a client that announces and then streams the same blobs
/// doesn't race the mailbox into a duplicate fetch; otherwise each hash is
/// fetchable immediately.
pub async fn record_blob_sources(
    blob_sync: &BlobSync,
    hashes: &[iroh_blobs::Hash],
    source: iroh::EndpointId,
    expect_upload: bool,
) {
    for hash in hashes {
        blob_sync
            .add_fetch_source(*hash, source, expect_upload)
            .await;
    }
}

/// Handle `POST /blobs/register-hashes`: partition the announced hashes into
/// those the mailbox already holds (reported back as `already_stored`) and those
/// it lacks (registered in the fetch pool so it pulls them from `sender_pubkey`).
/// This is the source of truth for what the mailbox has: a blob pushed to
/// `/blobs/upload` shows up here as `already_stored`, and anything that never
/// arrived falls into the fetch pool as a backstop.
pub async fn register_hashes(
    State(state): State<AppState>,
    Json(payload): Json<RegisterHashesRequest>,
) -> Result<Json<RegisterHashesResponse>, (StatusCode, String)> {
    // Record this operation's reference to each blob. Any hash whose reference
    // is already tombstoned drops out here and is neither stored nor fetched:
    // a node that has not yet processed the delete must not be able to
    // resurrect scrubbed media by re-announcing it.
    let db = state.db.clone();
    let hashes = payload.blob_hashes.clone();
    let op_ref = payload.op_ref.clone();
    let wanted = tokio::task::spawn_blocking(move || record_blob_refs(&db, &hashes, &op_ref))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?
        .map_err(|e| {
            tracing::error!("{}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?;

    let mut already_stored = Vec::new();
    let mut to_fetch = Vec::new();
    for hash in wanted {
        if state.blob_sync.blobs.has(hash).await.unwrap_or(false) {
            already_stored.push(hash);
        } else {
            to_fetch.push(hash);
        }
    }
    record_blob_sources(
        &state.blob_sync,
        &to_fetch,
        payload.sender_pubkey,
        payload.expect_upload,
    )
    .await;
    Ok(Json(RegisterHashesResponse { already_stored }))
}

/// Handle `POST /blobs/upload`: store the raw blob bytes in the request body and
/// return the hash they hash to. This is the direct-upload fast path — clients
/// push bytes here immediately on publish so recipients get them without waiting
/// for the mailbox to dial back and fetch. It is best-effort from the client's
/// side: `/blobs/register-hashes` remains the source of truth, so a failed upload simply
/// falls back to a fetch. The 64MB `DefaultBodyLimit` caps a single upload.
pub async fn upload_blob(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadBlobResponse>, (StatusCode, String)> {
    // Refuse bytes whose every reference has been scrubbed, so an upload racing
    // a delete can't put the media back. Checked before storing, since the
    // upload carries no reference of its own to record.
    let offered = iroh_blobs::Hash::new(&body);
    let db = state.db.clone();
    let wanted = tokio::task::spawn_blocking(move || blob_is_wanted(&db, &offered))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?
        .map_err(|e| {
            tracing::error!("{}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?;
    if !wanted {
        tracing::debug!(hash = %offered, "rejecting upload of a scrubbed blob");
        return Ok(Json(UploadBlobResponse { hash: offered }));
    }

    let hash = state
        .blob_sync
        .store_pushed_blob(body)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    // The mailbox now holds this blob, so drop any pending fetch an earlier
    // announce queued for it and push out the grace window for the sender's
    // other still-deferred fetches (upload progress defers the fetch backstop).
    state.blob_sync.note_upload(hash).await;
    Ok(Json(UploadBlobResponse { hash }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test(start_paused = true)]
    async fn absent_blob_without_expected_upload_is_fetchable_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let h = iroh_blobs::Hash::new([9; 32]);

        // Announce with no expected upload: the source is fetchable immediately.
        record_blob_sources(&blob_sync, &[h], source, false).await;
        let tried = HashSet::new();
        let (got, sources) = blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .unwrap();
        assert_eq!(got, h);
        assert!(sources.contains(&source));
    }

    #[tokio::test(start_paused = true)]
    async fn expected_upload_defers_the_fetch_by_the_grace_window() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let h = iroh_blobs::Hash::new([9; 32]);

        // Announce with an expected upload: the fetch is held off until the grace
        // window passes, giving the upload time to land.
        record_blob_sources(&blob_sync, &[h], source, true).await;
        let tried = HashSet::new();
        assert!(blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .is_none());

        tokio::time::advance(
            crate::blob_sync::DEFAULT_UPLOAD_GRACE + std::time::Duration::from_secs(1),
        )
        .await;
        let (got, sources) = blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .unwrap();
        assert_eq!(got, h);
        assert!(sources.contains(&source));
    }

    #[tokio::test(start_paused = true)]
    async fn uploading_a_blob_clears_its_pending_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let data = bytes::Bytes::from_static(b"raced blob");
        let h = iroh_blobs::Hash::new(&data);

        // A hash is announced with an expected upload (queued for fetch after the
        // grace window)...
        record_blob_sources(&blob_sync, &[h], source, true).await;
        // ...then the client's upload lands, which must drop the pending fetch.
        blob_sync.store_pushed_blob(data.clone()).await.unwrap();
        blob_sync.note_upload(h).await;

        tokio::time::advance(
            crate::blob_sync::DEFAULT_UPLOAD_GRACE + std::time::Duration::from_secs(1),
        )
        .await;
        assert!(blob_sync
            .fetch_pool_for_test()
            .next_untried(&HashSet::new())
            .await
            .is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn pushed_blob_is_stored_under_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();

        let data = bytes::Bytes::from_static(b"pushed blob contents");
        let stored = blob_sync.store_pushed_blob(data.clone()).await.unwrap();

        assert_eq!(stored, iroh_blobs::Hash::new(&data));
        assert!(blob_sync.blobs.has(stored).await.unwrap());
    }
}
