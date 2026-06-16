use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::api::downloader::{Downloader, Shuffled};
use mailbox_client::manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, Notify},
    task::{JoinHandle, JoinSet},
};
use tokio_stream::StreamExt;

use crate::{AsBody, ChatPayload, Payload, mailbox::MailboxOperation, stores::OpStore};

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
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;
        let downloader = blobs.downloader(&endpoint.endpoint().await?);

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
            downloader,
        })
    }

    /// Spawn the background loop that drains the fetch pool, returning a handle
    /// that cancels the loop when aborted.
    ///
    /// Each pass walks the whole stack from the top with up to `concurrency`
    /// downloads in flight, giving every blob until `attempt_timeout` to arrive
    /// before moving on. Successfully stored blobs are removed from the stack.
    /// When a pass finishes with items still outstanding, the loop waits until
    /// `pass_interval` has elapsed since the pass began before trying again, but
    /// a newly added hash wakes it early so fresh blobs are attempted at once.
    pub fn spawn_fetch_loop(
        &self,
        concurrency: usize,
        attempt_timeout: Duration,
        pass_interval: Duration,
    ) -> JoinHandle<()> {
        let this = self.clone();
        let concurrency = concurrency.max(1);
        tokio::spawn(async move {
            loop {
                let pass_start = Instant::now();
                this.run_fetch_pass(concurrency, attempt_timeout).await;

                if this.fetch_pool.is_empty().await {
                    this.fetch_pool.added.notified().await;
                    continue;
                }

                let elapsed = pass_start.elapsed();
                if elapsed < pass_interval {
                    tokio::select! {
                        _ = tokio::time::sleep(pass_interval - elapsed) => {}
                        _ = this.fetch_pool.added.notified() => {}
                    }
                }
            }
        })
    }

    async fn run_fetch_pass(&self, concurrency: usize, attempt_timeout: Duration) {
        let mut tried: HashSet<iroh_blobs::Hash> = HashSet::new();
        let mut in_flight: JoinSet<Option<(LogId, iroh_blobs::Hash)>> = JoinSet::new();

        loop {
            while in_flight.len() < concurrency {
                let Some((log_id, hash)) = self.fetch_pool.next_untried(&tried).await else {
                    break;
                };
                tried.insert(hash);
                let this = self.clone();
                in_flight.spawn(async move { this.try_fetch(log_id, hash, attempt_timeout).await });
            }

            let Some(joined) = in_flight.join_next().await else {
                break;
            };
            if let Ok(Some((log_id, hash))) = joined {
                self.fetch_pool.remove(log_id, hash).await;
            }
        }
    }

    /// Attempt to fetch a single blob, returning `Some` when it is present in
    /// the local store afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        log_id: LogId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> Option<(LogId, iroh_blobs::Hash)> {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return Some((log_id, hash));
        }

        let sources = self.sources.sources(log_id).await.ok()?;
        if sources.is_empty() {
            return None;
        }

        let download = self.downloader.download(hash, Shuffled::new(sources));
        match tokio::time::timeout(attempt_timeout, download).await {
            Ok(Ok(())) => Some((log_id, hash)),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(LogId, iroh_blobs::Hash)>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        self.stack.lock().await.push((log_id, hash));
        self.added.notify_one();
    }

    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }

    /// Return the topmost (most recently added) entry whose hash has not yet
    /// been attempted this pass, so freshly added blobs are tried first.
    async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(LogId, iroh_blobs::Hash)> {
        let stack = self.stack.lock().await;
        stack
            .iter()
            .rev()
            .find(|(_, hash)| !tried.contains(hash))
            .copied()
    }

    async fn remove(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        self.stack
            .lock()
            .await
            .retain(|entry| entry != &(log_id, hash));
    }

    // TODO: can we just have a p2panda stream of all past and future operations?
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.stack.lock().await;
        tokio::pin!(ops);
        while let Some(op) = ops.try_next().await? {
            let Some(body) = op.body else {
                continue;
            };
            let payload = Payload::try_from_body(&body)?;
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    if let Some(media) = m.media_meta() {
                        for item in media {
                            s.push((op.header.extensions.log_id, item.hash));
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
}

impl MixedSourceLookup {
    pub async fn sources(&self, log_id: LogId) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let sources = self
            .op_store
            .get_authors(log_id)
            .await?
            .into_iter()
            .map(|author| iroh::EndpointId::from_bytes(author.as_bytes()))
            .collect::<Result<Vec<iroh::EndpointId>, _>>()?;
        // sources.extend(self.mailboxes.get_sources(log_id).await?);
        Ok(sources)
    }
}
