//! Manages syncing blobs referenced in logs over iroh-blobs

use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::api::downloader::{DownloadProgressItem, Downloader, Shuffled};
use iroh_blobs::protocol::GetRequest;
use mailbox_client::manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use dashchat_utils::FetchPool;

pub use dashchat_utils::FetchConfig as BlobFetchConfig;

use crate::{AsBody, ChatPayload, Payload, TopicId, mailbox::MailboxOperation, stores::OpStore};

/// Max size of a single blob. The hash is content-addressed but its size is not
/// bounded by anything the fetcher can see, so an untrusted log could reference
/// a blob far larger than any legitimate message (the composer caps a whole
/// message at 128 MiB, and each media item is a separate blob, so no single blob
/// can legitimately exceed it). Enforced both when fetching ([`download_capped`])
/// and when an honest node publishes its own media (`store_media`).
pub(crate) const MAX_BLOB_BYTES: u64 = 128 * 1024 * 1024;

/// Download `hash` from `providers`, aborting if the transfer exceeds
/// [`MAX_BLOB_BYTES`]. Returns whether the blob is present locally afterwards.
async fn download_capped(
    downloader: &Downloader,
    hash: iroh_blobs::Hash,
    providers: Shuffled,
    attempt_timeout: Duration,
    blobs: &iroh_blobs::BlobsProtocol,
) -> bool {
    let result = tokio::time::timeout(attempt_timeout, async {
        let mut stream = downloader
            .download(GetRequest::all(hash), providers)
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
        Ok(Ok(())) => blobs.has(hash).await.unwrap_or(false),
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

/// Manages syncing blobs referenced in logs over iroh-blobs
#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
    pub sources: MixedSourceLookup,
    downloader: Downloader,
}

impl BlobSync {
    pub async fn new(
        endpoint: p2panda::Endpoint,
        root: PathBuf,
        blob_fetch: BlobFetchPool,
        sources: MixedSourceLookup,
    ) -> anyhow::Result<Self> {
        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        endpoint
            .accept_unmixed(iroh_blobs::ALPN, blobs.clone())
            .await?;
        let downloader = Downloader::new(&store, &endpoint.endpoint().await?);

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
            downloader,
        })
    }

    /// Clone the downloader so an in-process mailbox can fetch blobs into this
    /// node's shared blob store.
    pub fn downloader(&self) -> Downloader {
        self.downloader.clone()
    }

    /// Spawn the background loop that drains the fetch pool, returning a handle
    /// that cancels the loop when aborted.
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(dashchat_utils::fetch_loop(
            pool,
            config,
            move |(topic, hash), attempt_timeout| {
                let this = this.clone();
                async move { this.try_fetch(topic, hash, attempt_timeout).await }
            },
        ))
    }

    /// Attempt to fetch a single blob, returning `true` when it is present in
    /// the local store afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        topic: TopicId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }

        let sources = match self.sources.sources(topic).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::warn!(%hash, ?err, "blob source lookup failed");
                return false;
            }
        };

        if sources.is_empty() {
            return false;
        }

        let providers = Shuffled::new(sources.into_iter().map(Into::into).collect());
        download_capped(
            &self.downloader,
            hash,
            providers,
            attempt_timeout,
            &self.blobs,
        )
        .await
    }

    /// Keep attempting an on-demand download of `hash` until it is present
    /// locally or `timeout` elapses, bypassing the background loop's long pass
    /// interval (up to a minute away). Retries within the window so a
    /// fast-failing attempt — e.g. a momentarily unreachable provider — gets
    /// another chance instead of leaving the caller to wait out the window.
    /// Tries every topic the pool associates with the hash; concurrent
    /// downloads of the same hash are coalesced by the iroh-blobs downloader,
    /// so racing the background loop is safe.
    pub async fn fetch_now(&self, hash: iroh_blobs::Hash, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.blobs.has(hash).await.unwrap_or(false) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let topics = self.fetch_pool.topics_for(hash).await;
            // No known source yet; the caller's poll still waits out the window
            // in case the blob arrives via the background loop or a mailbox.
            if topics.is_empty() {
                return false;
            }
            for topic in topics {
                if self.try_fetch(topic, hash, remaining).await {
                    return true;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(TopicId, iroh_blobs::Hash)>>>,
    added: Arc<Notify>,
}

#[async_trait::async_trait]
impl FetchPool for BlobFetchPool {
    type Item = (TopicId, iroh_blobs::Hash);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.1
    }
    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }
    async fn next_untried(&self, tried: &HashSet<iroh_blobs::Hash>) -> Option<Self::Item> {
        let stack = self.stack.lock().await;
        stack
            .iter()
            .rev()
            .find(|(_, hash)| !tried.contains(hash))
            .copied()
    }
    async fn remove(&self, item: &Self::Item) {
        self.stack.lock().await.retain(|entry| entry != item);
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

impl BlobFetchPool {
    pub async fn add(&self, topic: TopicId, hash: iroh_blobs::Hash) {
        self.stack.lock().await.push((topic, hash));
        self.added.notify_one();
    }

    /// Topics the pool currently associates with `hash`, used to resolve blob
    /// sources for an on-demand fetch.
    pub async fn topics_for(&self, hash: iroh_blobs::Hash) -> Vec<TopicId> {
        self.stack
            .lock()
            .await
            .iter()
            .filter(|(_, h)| *h == hash)
            .map(|(topic, _)| *topic)
            .collect()
    }

    /// Build a fetch pool from a stream of stored operations.
    ///
    /// The `topic_for_log_id` closure maps each operation's log_id back to a
    /// `TopicId`. `LogId = blake3(topic.as_bytes())` is one-way, so callers
    /// must supply this mapping from their own store. Returns `None` to skip
    /// an operation whose topic cannot be recovered.
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
        topic_for_log_id: impl Fn(&LogId) -> Option<TopicId>,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.stack.lock().await;
        tokio::pin!(ops);
        while let Some(op) = ops.try_next().await? {
            let Some(body) = op.body else {
                continue;
            };
            let Ok(payload) = Payload::try_from_body(&body) else {
                continue;
            };
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    if let Some(media) = m.media() {
                        let Some(topic) = topic_for_log_id(&op.header.extensions.log_id) else {
                            continue;
                        };
                        for item in media {
                            s.push((topic, item.hash));
                        }
                    }
                }
                _ => continue,
            }
        }
        drop(s);
        Ok(store)
    }
}

#[derive(Clone, Constructor)]
pub struct MixedSourceLookup {
    op_store: OpStore,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
    self_endpoint: iroh::EndpointId,
}

impl MixedSourceLookup {
    pub async fn sources(&self, topic: TopicId) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let log_id = LogId::from_topic(topic);
        let mut sources = self
            .op_store
            .get_authors(log_id)
            .await?
            .into_iter()
            .map(|author| iroh::EndpointId::from_bytes(author.as_bytes()))
            .collect::<Result<Vec<iroh::EndpointId>, _>>()?;
        sources.extend(self.mailboxes.get_sources(&topic).await?);
        // Never dial ourselves (we already early-return when the blob is local),
        // and dedupe so a provider isn't dialed twice — redundant dials churn
        // iroh connection paths.
        let mut seen = HashSet::new();
        sources.retain(|id| *id != self.self_endpoint && seen.insert(*id));
        Ok(sources)
    }
}
