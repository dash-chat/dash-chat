use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashchat_utils::{fetch_loop, FetchConfig, FetchStack};
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh_blobs::api::downloader::{Downloader, Shuffled};
use iroh_blobs::protocol::GetRequest;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    sources: Arc<Mutex<BTreeMap<iroh_blobs::Hash, BTreeSet<iroh::EndpointId>>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId) {
        self.sources.lock().await.entry(hash).or_default().insert(source);
        self.added.notify_one();
    }
    pub(crate) async fn is_empty(&self) -> bool {
        self.sources.lock().await.is_empty()
    }
    pub(crate) async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)> {
        let map = self.sources.lock().await;
        map.iter()
            .find(|(hash, _)| !tried.contains(*hash))
            .map(|(hash, sources)| (*hash, sources.iter().copied().collect()))
    }
    pub(crate) async fn remove(&self, hash: iroh_blobs::Hash) {
        self.sources.lock().await.remove(&hash);
    }
}

#[async_trait::async_trait]
impl FetchStack for BlobFetchPool {
    type Item = (iroh_blobs::Hash, Vec<iroh::EndpointId>);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.0
    }
    async fn is_empty(&self) -> bool {
        BlobFetchPool::is_empty(self).await
    }
    async fn next_untried(&self, tried: &HashSet<iroh_blobs::Hash>) -> Option<Self::Item> {
        BlobFetchPool::next_untried(self, tried).await
    }
    async fn remove(&self, item: &Self::Item) {
        BlobFetchPool::remove(self, item.0).await;
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub(crate) fetch_pool: BlobFetchPool,
    downloader: Downloader,
    endpoint_id: iroh::EndpointId,
    /// Held only when this BlobSync owns its iroh endpoint (standalone server).
    /// `None` when sharing an in-process node's endpoint, in which case the
    /// node keeps the endpoint, router, and blob store alive.
    _endpoint: Option<iroh::Endpoint>,
    _router: Option<Router>,
}

impl BlobSync {
    pub async fn new(secret_key: iroh::SecretKey, root: PathBuf) -> anyhow::Result<Self> {
        let endpoint = iroh::Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .bind()
            .await?;

        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();
        let downloader = Downloader::new(&store, &endpoint);
        let endpoint_id = endpoint.id();

        Ok(Self {
            blobs,
            fetch_pool: BlobFetchPool::default(),
            downloader,
            endpoint_id,
            _endpoint: Some(endpoint),
            _router: Some(router),
        })
    }

    /// Build a mailbox BlobSync that shares an existing iroh endpoint and blob
    /// store (the in-process node's) instead of creating its own. Relayed blobs
    /// land in the shared store and are served by the node's existing protocol,
    /// so the mailbox's EndpointId is the node's EndpointId.
    pub fn shared(
        blobs: iroh_blobs::BlobsProtocol,
        downloader: Downloader,
        endpoint_id: iroh::EndpointId,
    ) -> Self {
        Self {
            blobs,
            fetch_pool: BlobFetchPool::default(),
            downloader,
            endpoint_id,
            _endpoint: None,
            _router: None,
        }
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint_id
    }

    pub fn fetch_pool(&self) -> &BlobFetchPool {
        &self.fetch_pool
    }

    #[cfg(test)]
    pub fn fetch_pool_for_test(&self) -> BlobFetchPool {
        self.fetch_pool.clone()
    }

    pub fn spawn_fetch_loop(&self, config: FetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(fetch_loop(pool, config, move |(hash, sources), timeout| {
            let this = this.clone();
            async move { this.try_fetch(hash, sources, timeout).await }
        }))
    }

    async fn try_fetch(
        &self,
        hash: iroh_blobs::Hash,
        sources: Vec<iroh::EndpointId>,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }
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
            Ok(Ok(_)) => true,
            Ok(Err(err)) => {
                tracing::debug!(%hash, ?err, "mailbox blob download failed");
                false
            }
            Err(_) => {
                tracing::warn!(%hash, "mailbox blob download timed out");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_id_matches_secret_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let expected = key.public();
        let bs = BlobSync::new(key, dir.path().to_path_buf()).await.unwrap();
        assert_eq!(bs.endpoint_id(), expected);
    }

    use std::collections::HashSet;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }
    fn endpoint_id(n: u8) -> iroh::EndpointId {
        iroh::SecretKey::from_bytes(&[n; 32]).public()
    }

    #[tokio::test]
    async fn pool_dedupes_sources_per_hash() {
        let pool = BlobFetchPool::default();
        pool.add_source(hash(1), endpoint_id(10)).await;
        pool.add_source(hash(1), endpoint_id(11)).await;
        pool.add_source(hash(1), endpoint_id(10)).await; // dup source
        let tried = HashSet::new();
        let (h, sources) = pool.next_untried(&tried).await.unwrap();
        assert_eq!(h, hash(1));
        assert_eq!(sources.len(), 2);
    }

    #[tokio::test]
    async fn pool_remove_drops_the_hash() {
        let pool = BlobFetchPool::default();
        pool.add_source(hash(1), endpoint_id(1)).await;
        pool.remove(hash(1)).await;
        assert!(pool.is_empty().await);
    }
}
