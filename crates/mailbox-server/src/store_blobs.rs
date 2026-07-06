use axum::{body::Bytes, extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::{AppState, BlobSync};

/// A client's request to announce blob hashes to the mailbox. For each hash the
/// mailbox already holds it reports `already_stored`; every other hash it
/// registers in its fetch pool so it pulls the blob from `sender_pubkey`. Blob
/// bytes are never carried here — clients push those separately to
/// `/blobs/upload` and use this announce to reconcile what actually landed.
#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    /// The peer the mailbox should dial to fetch any hash it does not already
    /// hold (typically the sending client itself).
    pub sender_pubkey: iroh::EndpointId,
    /// Reserved for a `sender_pubkey` signature over the request; currently
    /// unverified, so clients send it empty. `#[serde(default)]` keeps older
    /// payloads that omit the field entirely decodable.
    #[serde(default)]
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsResponse {
    /// Hashes the mailbox already has stored (empty if it has stored none).
    pub already_stored: Vec<iroh_blobs::Hash>,
}

#[derive(Serialize, Deserialize)]
pub struct UploadBlobResponse {
    /// The hash the uploaded bytes hash to, so the client can confirm the
    /// mailbox stored what it intended.
    pub hash: iroh_blobs::Hash,
}

/// Register `source` as a provider for each hash the mailbox does not yet hold,
/// deferring each fetch by the upload grace window so a client that announces and
/// then streams the same blobs doesn't race the mailbox into a duplicate fetch.
pub async fn record_blob_sources(
    blob_sync: &BlobSync,
    hashes: &[iroh_blobs::Hash],
    source: iroh::EndpointId,
) {
    for hash in hashes {
        blob_sync.add_delayed_fetch_source(*hash, source).await;
    }
}

/// Handle `POST /blobs/store`: partition the announced hashes into those the
/// mailbox already holds (reported back as `already_stored`) and those it lacks
/// (registered in the fetch pool so it pulls them from `sender_pubkey`). This is
/// the source of truth for what the mailbox has: a blob pushed to `/blobs/upload`
/// shows up here as `already_stored`, and anything that never arrived falls into
/// the fetch pool as a backstop.
pub async fn store_blobs(
    State(state): State<AppState>,
    Json(payload): Json<StoreBlobsRequest>,
) -> Json<StoreBlobsResponse> {
    let mut already_stored = Vec::new();
    let mut to_fetch = Vec::new();
    for hash in payload.blob_hashes {
        if state.blob_sync.blobs.has(hash).await.unwrap_or(false) {
            already_stored.push(hash);
        } else {
            to_fetch.push(hash);
        }
    }
    record_blob_sources(&state.blob_sync, &to_fetch, payload.sender_pubkey).await;
    Json(StoreBlobsResponse { already_stored })
}

/// Handle `POST /blobs/upload`: store the raw blob bytes in the request body and
/// return the hash they hash to. This is the direct-upload fast path — clients
/// push bytes here immediately on publish so recipients get them without waiting
/// for the mailbox to dial back and fetch. It is best-effort from the client's
/// side: `/blobs/store` remains the source of truth, so a failed upload simply
/// falls back to a fetch. The 64MB `DefaultBodyLimit` caps a single upload.
pub async fn upload_blob(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadBlobResponse>, (StatusCode, String)> {
    let hash = state
        .blob_sync
        .store_pushed_blob(body)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    // The mailbox now holds this blob, so drop any pending fetch an earlier
    // announce queued for it.
    state.blob_sync.clear_pending_fetch(hash).await;
    Ok(Json(UploadBlobResponse { hash }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test(start_paused = true)]
    async fn absent_blob_registers_source_and_is_not_already_stored() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let h = iroh_blobs::Hash::new([9; 32]);

        record_blob_sources(&blob_sync, &[h], source).await;
        let tried = HashSet::new();

        // The announce defers the fetch by the grace window, so it is not yet
        // ready to dial the source.
        assert!(blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .is_none());

        // Once the grace window passes, the source is fetchable.
        tokio::time::advance(crate::blob_sync::FETCH_GRACE + std::time::Duration::from_secs(1))
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

        // A hash is announced (queued for fetch after the grace window)...
        record_blob_sources(&blob_sync, &[h], source).await;
        // ...then the client's upload lands, which must drop the pending fetch.
        blob_sync.store_pushed_blob(data.clone()).await.unwrap();
        blob_sync.clear_pending_fetch(h).await;

        tokio::time::advance(crate::blob_sync::FETCH_GRACE + std::time::Duration::from_secs(1))
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
