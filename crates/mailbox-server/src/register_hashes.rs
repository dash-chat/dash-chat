use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{AppState, BlobSync};

/// A client's request to announce blob hashes to the mailbox. For each hash the
/// mailbox already holds it reports `already_stored`; every other hash it
/// registers in its fetch pool so it pulls the blob from `sender_pubkey`. Blob
/// bytes are never carried here — clients push those separately to
/// `/blobs/upload` and use this announce to reconcile what actually landed.
#[derive(Serialize, Deserialize)]
pub struct RegisterHashesRequest {
    pub blob_hashes: Vec<iroh_blobs::Hash>,
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

#[derive(Serialize, Deserialize)]
pub struct BlobStoredResponse {
    /// Whether the mailbox currently has this blob stored.
    pub stored: bool,
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
) -> Json<RegisterHashesResponse> {
    let mut already_stored = Vec::new();
    let mut to_fetch = Vec::new();
    for hash in payload.blob_hashes {
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
    Json(RegisterHashesResponse { already_stored })
}

/// Handle `GET /blobs/:hash/stored`: read-only check whether the mailbox has
/// already stored the given blob.
pub async fn query_blob_stored(
    State(state): State<AppState>,
    Path(hash): Path<iroh_blobs::Hash>,
) -> Json<BlobStoredResponse> {
    let stored = state.blob_sync.blobs.has(hash).await.unwrap_or(false);
    Json(BlobStoredResponse { stored })
}

/// Handle `POST /blobs/upload`: store the raw blob bytes in the request body and
/// return the hash they hash to. This is the direct-upload fast path — clients
/// push bytes here immediately on publish so recipients get them without waiting
/// for the mailbox to dial back and fetch. It is best-effort from the client's
/// side: `/blobs/register-hashes` remains the source of truth, so a failed upload simply
/// falls back to a fetch. Uploads past the 16 MiB blob cap are rejected — the
/// same limit the fetch and iroh-push paths enforce.
pub async fn upload_blob(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadBlobResponse>, (StatusCode, String)> {
    // An oversized blob is undeliverable receiver-side (`download_capped`
    // refuses it), so retaining it would only serve an abusive sender.
    if body.len() as u64 > dashchat_utils::blob_sync::MAX_BLOB_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "blob exceeds the {} byte cap",
                dashchat_utils::blob_sync::MAX_BLOB_BYTES
            ),
        ));
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
        let blob_sync = crate::BlobSync::new(
            key,
            dir.path().to_path_buf(),
            None,
            *dashchat_utils::NETWORK_ID,
        )
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
        let blob_sync = crate::BlobSync::new(
            key,
            dir.path().to_path_buf(),
            None,
            *dashchat_utils::NETWORK_ID,
        )
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
        let blob_sync = crate::BlobSync::new(
            key,
            dir.path().to_path_buf(),
            None,
            *dashchat_utils::NETWORK_ID,
        )
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
        let blob_sync = crate::BlobSync::new(
            key,
            dir.path().to_path_buf(),
            None,
            *dashchat_utils::NETWORK_ID,
        )
        .await
        .unwrap();

        let data = bytes::Bytes::from_static(b"pushed blob contents");
        let stored = blob_sync.store_pushed_blob(data.clone()).await.unwrap();

        assert_eq!(stored, iroh_blobs::Hash::new(&data));
        assert!(blob_sync.blobs.has(stored).await.unwrap());
    }

    #[tokio::test]
    async fn query_blob_stored_reports_true_after_upload_and_false_before() {
        let (server, _tmp) = crate::test_utils::create_test_server().await;
        let data = bytes::Bytes::from_static(b"probed blob");
        let hash = iroh_blobs::Hash::new(&data);

        let before = server.get(&format!("/blobs/{hash}/stored")).await;
        before.assert_status_ok();
        assert!(!before.json::<BlobStoredResponse>().stored);

        server
            .post("/blobs/upload")
            .bytes(data)
            .await
            .assert_status_ok();

        let after = server.get(&format!("/blobs/{hash}/stored")).await;
        after.assert_status_ok();
        assert!(after.json::<BlobStoredResponse>().stored);
    }

    #[tokio::test]
    async fn query_blob_stored_does_not_register_a_source() {
        let (db, _temp_file) = crate::test_utils::create_test_db();
        let push_tasks = std::sync::Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new()));
        let blob_sync = crate::test_utils::test_blob_sync().await;
        let app = crate::create_app(
            std::sync::Arc::new(db),
            None,
            push_tasks,
            blob_sync.clone(),
            crate::ServerConfig::default(),
        );
        let config = axum_test::TestServerConfig {
            transport: Some(axum_test::Transport::HttpRandomPort),
            ..axum_test::TestServerConfig::default()
        };
        let server = axum_test::TestServer::new_with_config(app, config).unwrap();
        let hash = iroh_blobs::Hash::new([42u8; 32]);

        server
            .get(&format!("/blobs/{hash}/stored"))
            .await
            .assert_status_ok();

        // A source would only be registered by /blobs/register-hashes, so the
        // fetch pool must stay empty after the probe.
        assert!(blob_sync
            .fetch_pool_for_test()
            .next_untried(&std::collections::HashSet::new())
            .await
            .is_none());
    }

    #[tokio::test]
    async fn upload_at_the_blob_cap_is_stored() {
        let (server, _tmp) = crate::test_utils::create_test_server().await;
        let body = vec![7u8; dashchat_utils::blob_sync::MAX_BLOB_BYTES as usize];
        let expected = iroh_blobs::Hash::new(&body);
        let response = server.post("/blobs/upload").bytes(body.into()).await;
        response.assert_status_ok();
        assert_eq!(response.json::<UploadBlobResponse>().hash, expected);
    }

    #[tokio::test]
    async fn upload_over_the_blob_cap_is_rejected() {
        let (server, _tmp) = crate::test_utils::create_test_server().await;
        let body = vec![7u8; dashchat_utils::blob_sync::MAX_BLOB_BYTES as usize + 1];
        let response = server.post("/blobs/upload").bytes(body.into()).await;
        response.assert_status(StatusCode::PAYLOAD_TOO_LARGE);
    }
}
