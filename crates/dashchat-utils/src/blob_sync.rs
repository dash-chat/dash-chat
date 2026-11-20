use std::sync::Arc;

use futures::StreamExt;
use iroh_blobs::api::downloader::{
    ContentDiscovery, DownloadProgressItem, DownloadRequest, Downloader, FiniteRequest,
    SplitStrategy,
};
use iroh_blobs::protocol::GetRequest;
use tokio::time::Duration;

/// Max size of a single blob. The hash is content-addressed but its size is not
/// bounded by anything the fetcher can see, so an untrusted log could reference
/// a blob far larger than any legitimate message (the composer caps a whole
/// message at 16 MiB, and each media item is a separate blob, so no single blob
/// can legitimately exceed it). Enforced both when fetching ([`download_capped`])
/// and when an honest node publishes its own media (`store_media`).
pub const MAX_BLOB_BYTES: u64 = 16 * 1024 * 1024;

/// AEAD tag added by blob encryption: a stored ciphertext is this much larger
/// than its plaintext, and must still fit under [`MAX_BLOB_BYTES`].
pub const BLOB_ENCRYPTION_OVERHEAD: u64 = 16;

/// Download `hash` from `providers`, aborting if the transfer exceeds
/// [`MAX_BLOB_BYTES`]. Returns whether the blob is present locally afterwards.
pub async fn download_capped(
    downloader: &Downloader,
    hash: iroh_blobs::Hash,
    providers: impl ContentDiscovery,
    attempt_timeout: Duration,
    blobs: &iroh_blobs::BlobsProtocol,
) -> bool {
    let result = tokio::time::timeout(attempt_timeout, async {
        let options = DownloadRequest {
            // Media are single blobs, not hash-sequences. `GetRequest::blob`
            // requests only the blob itself; `GetRequest::all` would additionally
            // request the blob's hash-sequence children, which makes the provider
            // parse the raw media bytes as a hash-seq and reset the stream with
            // `ERR_INTERNAL` (`InvalidHashSeq`) — so the blob never transfers.
            request: FiniteRequest::Get(GetRequest::blob(hash)),
            providers: Arc::new(providers),
            // `SplitStrategy::Split` routes the download through iroh-blobs'
            // hash-seq path, which asserts the root size is a multiple of 32 and
            // so always fails for a raw blob ("Size is not a multiple of 32").
            strategy: SplitStrategy::None,
        };
        let mut stream = downloader
            .download_with_opts(options)
            .stream()
            .await
            .map_err(|e| anyhow::anyhow!("download stream: {e}"))?;
        while let Some(item) = stream.next().await {
            match item {
                // Dropping the stream on return cancels the in-flight download.
                DownloadProgressItem::Progress(total) if total > MAX_BLOB_BYTES => {
                    anyhow::bail!("blob exceeds {MAX_BLOB_BYTES} byte cap ({total} bytes)")
                }
                DownloadProgressItem::Error(err) => anyhow::bail!("download failed: {err}"),
                DownloadProgressItem::DownloadError => anyhow::bail!("download error"),
                _ => {}
            }
        }
        anyhow::Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => {
            let present = blobs.has(hash).await.unwrap_or(false);
            if !present {
                tracing::debug!(%hash, "blob download ended without the blob");
            }
            present
        }
        Ok(Err(err)) => {
            tracing::debug!(%hash, ?err, "blob download failed");
            false
        }
        Err(_) => {
            tracing::warn!(%hash, "blob download timed out");
            false
        }
    }
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    /// The UI derives its attachment cap from `ui/src/lib/utils/media-limits.json`,
    /// which mirrors the constants above; this fails when either side drifts.
    #[test]
    fn ui_media_limits_match() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/lib/utils/media-limits.json"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(json["max_blob_bytes"], MAX_BLOB_BYTES);
        assert_eq!(json["blob_encryption_overhead"], BLOB_ENCRYPTION_OVERHEAD);
    }
}
