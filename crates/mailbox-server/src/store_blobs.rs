use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{AppState, BlobSync};

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    pub sender_pubkey: iroh::EndpointId,
    #[serde(default)]
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsResponse {
    /// Hashes the mailbox already has stored (empty if it has stored none).
    pub already_stored: Vec<iroh_blobs::Hash>,
}

/// Register `source` as a provider for each hash the mailbox does not yet hold.
pub async fn record_blob_sources(
    blob_sync: &BlobSync,
    hashes: &[iroh_blobs::Hash],
    source: iroh::EndpointId,
) {
    for hash in hashes {
        blob_sync.fetch_pool().add_source(*hash, source).await;
    }
}

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
        let (got, sources) = blob_sync
            .fetch_pool_for_test()
            .next_untried(&tried)
            .await
            .unwrap();
        assert_eq!(got, h);
        assert!(sources.contains(&source));
    }
}
