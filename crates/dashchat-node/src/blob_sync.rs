use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::api::downloader::{Downloader, Shuffled};
use iroh_blobs::protocol::GetRequest;
use mailbox_client::manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify},
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use dashchat_utils::FetchStack;

pub use dashchat_utils::FetchConfig as BlobFetchConfig;

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

    /// Spawn the background loop that drains the fetch pool, returning a handle
    /// that cancels the loop when aborted.
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(dashchat_utils::fetch_loop(
            pool,
            config,
            move |(log_id, hash), attempt_timeout| {
                let this = this.clone();
                async move { this.try_fetch(log_id, hash, attempt_timeout).await }
            },
        ))
    }

    /// Attempt to fetch a single blob, returning `true` when it is present in
    /// the local store afterwards (already cached or newly downloaded).
    async fn try_fetch(
        &self,
        log_id: LogId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }

        let sources = match self.sources.sources(log_id).await {
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

        match tokio::time::timeout(
            attempt_timeout,
            self.downloader.download(GetRequest::all(hash), providers),
        )
        .await
        {
            Ok(Ok(_stats)) => true,
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
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(LogId, iroh_blobs::Hash)>>>,
    added: Arc<Notify>,
}

#[async_trait::async_trait]
impl FetchStack for BlobFetchPool {
    type Item = (LogId, iroh_blobs::Hash);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.1
    }
    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }
    async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<Self::Item> {
        let stack = self.stack.lock().await;
        stack.iter().rev().find(|(_, hash)| !tried.contains(hash)).copied()
    }
    async fn remove(&self, item: &Self::Item) {
        self.stack.lock().await.retain(|entry| entry != item);
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

impl BlobFetchPool {
    pub async fn add(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        self.stack.lock().await.push((log_id, hash));
        self.added.notify_one();
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
