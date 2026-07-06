use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{AppState, BlobSync};

/// A client's request to make blobs available on the mailbox. Clients send this
/// two ways: first with the blob bytes inline (`blobs = Some`) so the mailbox can
/// store them without a round trip back to the client, and — if that upload times
/// out — again with `blobs = None`, leaving the mailbox to fetch them from
/// `sender_pubkey` via its fetch pool.
#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    /// The blob bytes themselves, when the client uploads them inline. `None`
    /// means announce-only: the mailbox fetches any hashes it lacks from
    /// `sender_pubkey` instead. Bytes are matched to `blob_hashes` by content
    /// hash, not position, so order and extra/missing entries are harmless.
    pub blobs: Option<Vec<bytes::Bytes>>,
    /// Every blob hash this request is about, whether or not its bytes are
    /// included in `blobs`. Hashes the mailbox ends up without are added to the
    /// fetch pool.
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    /// The peer the mailbox should dial to fetch any hash it does not receive
    /// inline (typically the sending client itself).
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

/// Handle `POST /blobs/store`: persist any inline blob bytes, then partition the
/// announced hashes into those the mailbox now holds (reported back as
/// `already_stored`) and those it still lacks (registered in the fetch pool so it
/// pulls them from `sender_pubkey`). A hash sent with matching bytes lands in the
/// former group and is never fetched; a hash sent without bytes (or whose bytes
/// failed to store) lands in the latter. The `has` check does double duty here:
/// it catches both blobs stored moments ago from this request and blobs the
/// mailbox already had from a previous one.
pub async fn store_blobs(
    State(state): State<AppState>,
    Json(payload): Json<StoreBlobsRequest>,
) -> Json<StoreBlobsResponse> {
    if let Some(blobs) = payload.blobs {
        for blob in blobs {
            if let Err(err) = state.blob_sync.store_pushed_blob(blob).await {
                tracing::warn!(?err, "failed to store pushed blob");
            }
        }
    }
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
