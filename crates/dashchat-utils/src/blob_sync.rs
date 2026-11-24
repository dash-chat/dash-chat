use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::Stream;
use futures::StreamExt;
use iroh_blobs::api::downloader::{
    ContentDiscovery, DownloadProgressItem, DownloadRequest, Downloader, FiniteRequest,
    SplitStrategy,
};
use iroh_blobs::protocol::GetRequest;
use tokio::sync::watch;

pub type BoxedDownloadStream = Pin<Box<dyn Stream<Item = DownloadProgressItem> + Send + 'static>>;

/// Snapshot of an in-flight blob download, updated by the downloader as it makes
/// progress. Used by the caller to decide whether to abort a slow peer transfer
/// and pivot to a faster source.
#[derive(Clone, Copy, Debug, Default)]
pub struct DownloadProgress {
    /// The provider currently being attempted, if known.
    pub provider: Option<iroh::EndpointId>,
    /// Total bytes transferred so far for this blob.
    pub bytes_transferred: u64,
    /// When bytes_transferred last increased.
    pub last_progress_at: Option<Instant>,
    /// When the current provider was first reported.
    pub provider_changed_at: Option<Instant>,
}

impl DownloadProgress {
    /// Start tracking a new provider, preserving accumulated byte counters.
    fn provider_changed(mut self, provider: iroh::EndpointId, now: Instant) -> Self {
        self.provider = Some(provider);
        self.provider_changed_at = Some(now);
        self
    }

    /// Update cumulative bytes and the timestamp of the last forward progress.
    fn progress(mut self, bytes: u64, now: Instant) -> Self {
        self.bytes_transferred = bytes;
        self.last_progress_at = Some(now);
        self
    }
}

/// Abstracts the iroh-blobs downloader so tests can inject mock download streams
/// (slow transfers, provider failures, progress events) without real networking.
#[async_trait]
pub trait BlobDownloader: Send + Sync + 'static {
    async fn download_with_opts(
        &self,
        options: DownloadRequest,
    ) -> anyhow::Result<BoxedDownloadStream>;
}

#[async_trait]
impl BlobDownloader for Downloader {
    async fn download_with_opts(
        &self,
        options: DownloadRequest,
    ) -> anyhow::Result<BoxedDownloadStream> {
        let stream = self
            .download_with_opts(options)
            .stream()
            .await
            .map_err(|e| anyhow::anyhow!("download stream: {e}"))?;
        Ok(Box::pin(stream))
    }
}

/// Max size of a single blob. The hash is content-addressed but its size is not
/// bounded by anything the fetcher can see, so an untrusted log could reference
/// a blob far larger than any legitimate message (the composer caps a whole
/// message at 16 MiB, and each media item is a separate blob, so no single blob
/// can legitimately exceed it). Enforced both when fetching ([`download_capped`])
/// and when an honest node publishes its own media (`store_media`).
pub const MAX_BLOB_BYTES: u64 = 16 * 1024 * 1024;

/// Download `hash` from `providers`, aborting if the transfer exceeds
/// [`MAX_BLOB_BYTES`]. Returns whether the blob is present locally afterwards.
/// `progress_tx` is updated with the provider currently being attempted and
/// bytes transferred so the caller can observe stalls and near-completion.
pub async fn download_capped(
    downloader: &dyn BlobDownloader,
    hash: iroh_blobs::Hash,
    providers: impl ContentDiscovery,
    attempt_timeout: Duration,
    blobs: &iroh_blobs::BlobsProtocol,
    progress_tx: watch::Sender<DownloadProgress>,
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
            .await
            .map_err(|e| anyhow::anyhow!("download stream: {e}"))?;
        while let Some(item) = stream.next().await {
            let now = Instant::now();
            match item {
                // Dropping the stream on return cancels the in-flight download.
                DownloadProgressItem::Progress(total) if total > MAX_BLOB_BYTES => {
                    anyhow::bail!("blob exceeds {MAX_BLOB_BYTES} byte cap ({total} bytes)")
                }
                DownloadProgressItem::Error(err) => anyhow::bail!("download failed: {err}"),
                DownloadProgressItem::DownloadError => anyhow::bail!("download error"),
                DownloadProgressItem::Progress(total) => {
                    let next = progress_tx.borrow().progress(total, now);
                    let _ = progress_tx.send(next);
                }
                DownloadProgressItem::TryProvider { id, .. } => {
                    let next = progress_tx.borrow().provider_changed(id, now);
                    let _ = progress_tx.send(next);
                }
                DownloadProgressItem::ProviderFailed { id, .. } => {
                    let current = progress_tx.borrow().provider;
                    if current == Some(id) {
                        let _ = progress_tx.send(Default::default());
                    }
                }
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
