use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashchat_utils::{fetch_loop, FetchConfig, FetchPool, NETWORK_ID};
use futures::StreamExt;
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh_blobs::api::downloader::{Downloader, Shuffled};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

/// Tag-name prefix marking a stored blob the server is responsible for GCing.
/// The fetch time (unix seconds, zero-padded for lexical order) is embedded so
/// `expire_blob_tags` can drop tags past the retention window.
const BLOB_TAG_PREFIX: &str = "mailbox/";
/// Retention for stored blobs, matching the 7-day blip retention in cleanup.rs.
const BLOB_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// How often iroh sweeps untagged blobs and how often we expire stale tags.
const BLOB_GC_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// Drop a pending fetch entry after this many consecutive failed passes.
const MAX_FETCH_FAILURES: u32 = 10;
/// Hard cap on pending fetch entries; arbitrary entries are dropped if exceeded.
const MAX_POOL_SIZE: usize = 1_000;

#[derive(Default)]
struct PoolEntry {
    sources: BTreeSet<iroh::EndpointId>,
    failures: u32,
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    entries: Arc<Mutex<BTreeMap<iroh_blobs::Hash, PoolEntry>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId) {
        let mut map = self.entries.lock().await;
        if !map.contains_key(&hash) && map.len() >= MAX_POOL_SIZE {
            // Drop the first (lexicographically earliest) entry to stay within the cap.
            if let Some(oldest) = map.keys().next().copied() {
                map.remove(&oldest);
            }
        }
        map.entry(hash).or_default().sources.insert(source);
        self.added.notify_one();
    }

    pub(crate) async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }

    pub(crate) async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)> {
        let map = self.entries.lock().await;
        map.iter()
            .find(|(hash, _)| !tried.contains(*hash))
            .map(|(hash, entry)| (*hash, entry.sources.iter().copied().collect()))
    }

    pub(crate) async fn remove(&self, hash: iroh_blobs::Hash) {
        self.entries.lock().await.remove(&hash);
    }

    /// Record a failed fetch attempt; evict the entry after [`MAX_FETCH_FAILURES`].
    pub(crate) async fn record_failure(&self, hash: iroh_blobs::Hash) {
        let mut map = self.entries.lock().await;
        if let Some(entry) = map.get_mut(&hash) {
            entry.failures += 1;
            if entry.failures >= MAX_FETCH_FAILURES {
                map.remove(&hash);
                tracing::debug!(%hash, "evicting blob from fetch pool after too many failures");
            }
        }
    }
}

#[async_trait::async_trait]
impl FetchPool for BlobFetchPool {
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
    async fn on_failure(&self, item: &Self::Item) {
        BlobFetchPool::record_failure(self, item.0).await;
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

#[derive(Clone)]
enum PeerAddrRegistry {
    /// Standalone server: lookup is wired directly into the iroh endpoint builder.
    Memory(MemoryLookup),
    /// Shared (in-process) server: iroh endpoint is p2panda's; addresses are
    /// forwarded to the node's address book via an unbounded channel.
    Channel(UnboundedSender<iroh::EndpointAddr>),
}

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub(crate) fetch_pool: BlobFetchPool,
    downloader: Downloader,
    /// The iroh endpoint blobs are served from. Held both to keep a
    /// standalone server's endpoint alive and to read its live [`EndpointAddr`]
    /// (relay + direct addresses) for the `/health` response so clients can
    /// dial this mailbox by its EndpointId. In the shared model this is a clone
    /// of the in-process node's endpoint.
    endpoint: iroh::Endpoint,
    fetch_config: FetchConfig,
    /// True when this BlobSync owns its blob store (standalone server) and is
    /// therefore responsible for GCing stored blobs. False when sharing an
    /// in-process node's store, where the node owns blob lifecycle.
    enable_gc: bool,
    /// Held only when this BlobSync owns its iroh endpoint (standalone server).
    /// `None` when sharing an in-process node's endpoint, in which case the
    /// node keeps the router and blob store alive.
    _router: Option<Router>,
    peer_addr_registry: PeerAddrRegistry,
}

impl BlobSync {
    /// Build a standalone mailbox BlobSync that owns its own iroh endpoint and
    /// blob store. When `relay_url` is set the endpoint registers with that
    /// relay so it is reachable behind NAT and its advertised [`EndpointAddr`]
    /// includes the relay; the call waits (bounded) for the relay connection so
    /// the first `/health` response carries a complete address.
    pub async fn new(
        secret_key: iroh::SecretKey,
        root: PathBuf,
        relay_url: Option<iroh::RelayUrl>,
    ) -> anyhow::Result<Self> {
        let peer_addr_lookup = MemoryLookup::new();
        let mut builder = iroh::Endpoint::builder(presets::Minimal)
            .secret_key(secret_key)
            .address_lookup(peer_addr_lookup.clone());
        let has_relay = relay_url.is_some();
        if let Some(relay_url) = relay_url {
            builder = builder.relay_mode(iroh::RelayMode::Custom(iroh::RelayMap::from_iter([
                relay_url,
            ])));
        }
        let endpoint = builder.bind().await?;

        // `endpoint.addr()` only includes the relay once the endpoint has
        // connected to it, so wait for that before serving `/health`. Bounded
        // so an unreachable relay can't block server startup indefinitely.
        // Skipped when no relay is configured (e.g. tests): with the `Minimal`
        // preset there is no default relay, so `online()` would never resolve.
        if has_relay {
            if let Err(e) = tokio::time::timeout(Duration::from_secs(10), endpoint.online()).await {
                tracing::error!(
                    ?e,
                    "failed to connect to bind iroh endpoint after 10 seconds"
                );
                return Err(anyhow::anyhow!(
                    "failed to connect to bind iroh endpoint after 10 seconds: {e}"
                ));
            }
        }

        let db_path = root.join("blobs.db");
        let mut options = iroh_blobs::store::fs::options::Options::new(&root);
        options.gc = Some(iroh_blobs::store::GcConfig {
            interval: BLOB_GC_INTERVAL,
            add_protected: None,
        });
        let mixed_alpn =
            p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, *NETWORK_ID);
        let store = iroh_blobs::store::fs::FsStore::load_with_opts(db_path, options).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let downloader =
            Downloader::new_with_opts(&store, &endpoint, mixed_alpn.as_slice(), Default::default());
        let router = Router::builder(endpoint.clone())
            .accept(mixed_alpn, blobs.clone())
            .spawn();

        Ok(Self {
            blobs,
            fetch_pool: BlobFetchPool::default(),
            downloader,
            endpoint,
            fetch_config: FetchConfig::default(),
            enable_gc: true,
            _router: Some(router),
            peer_addr_registry: PeerAddrRegistry::Memory(peer_addr_lookup),
        })
    }

    /// Build a mailbox BlobSync that shares an existing iroh endpoint and blob
    /// store (the in-process node's) instead of creating its own. Relayed blobs
    /// land in the shared store and are served by the node's existing protocol,
    /// so the mailbox's EndpointId is the node's EndpointId and its advertised
    /// `EndpointAddr` is the node's.
    pub fn shared(
        blobs: iroh_blobs::BlobsProtocol,
        downloader: Downloader,
        endpoint: iroh::Endpoint,
        peer_addr_tx: UnboundedSender<iroh::EndpointAddr>,
    ) -> Self {
        Self {
            blobs,
            fetch_pool: BlobFetchPool::default(),
            downloader,
            endpoint,
            fetch_config: FetchConfig::default(),
            enable_gc: false,
            _router: None,
            peer_addr_registry: PeerAddrRegistry::Channel(peer_addr_tx),
        }
    }

    /// Override the fetch loop's cadence (concurrency, attempt timeout, retry
    /// interval). Used by `spawn_server` when it spawns the loop.
    pub fn with_fetch_config(mut self, config: FetchConfig) -> Self {
        self.fetch_config = config;
        self
    }

    pub(crate) fn fetch_config(&self) -> FetchConfig {
        self.fetch_config.clone()
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }

    /// The endpoint's current dialing address (relay + direct addresses),
    /// served via `/health` so clients can reach this mailbox by its EndpointId.
    pub fn endpoint_addr(&self) -> iroh::EndpointAddr {
        self.endpoint.addr()
    }

    /// Register a peer's dialing address so the blob downloader can reach it
    /// by its EndpointId.
    pub fn add_peer_addr(&self, addr: iroh::EndpointAddr) {
        match &self.peer_addr_registry {
            PeerAddrRegistry::Memory(lookup) => lookup.add_endpoint_info(addr),
            PeerAddrRegistry::Channel(tx) => {
                let _ = tx.send(addr);
            }
        }
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
        let fetched = dashchat_utils::blob_sync::download_capped(
            &self.downloader,
            hash,
            providers,
            attempt_timeout,
            &self.blobs,
        )
        .await;
        if fetched {
            self.protect_blob(hash).await;
        }
        fetched
    }

    /// Tag a freshly stored blob so iroh's GC keeps it; the tag name embeds the
    /// fetch time so [`expire_blob_tags`] can drop it after the retention window.
    /// No-op when sharing an in-process node's store (the node owns lifecycle).
    async fn protect_blob(&self, hash: iroh_blobs::Hash) {
        if !self.enable_gc {
            return;
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("{BLOB_TAG_PREFIX}{secs:020}/{hash}");
        if let Err(err) = self.blobs.store().tags().set(name, hash).await {
            tracing::warn!(%hash, ?err, "failed to tag stored blob for retention");
        }
    }

    /// Spawn the loop that expires stored-blob tags past the retention window;
    /// iroh's background GC then reclaims the now-untagged blobs. Returns `None`
    /// when sharing a node's store (the node owns blob lifecycle).
    pub fn spawn_blob_gc_task(&self) -> Option<JoinHandle<()>> {
        if !self.enable_gc {
            return None;
        }
        let blobs = self.blobs.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(BLOB_GC_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(err) = expire_blob_tags(&blobs).await {
                    tracing::error!(?err, "failed to expire stored blob tags");
                }
            }
        }))
    }
}

/// Delete stored-blob tags older than [`BLOB_RETENTION`] so iroh's GC reclaims
/// the underlying blobs on its next sweep.
async fn expire_blob_tags(blobs: &iroh_blobs::BlobsProtocol) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(BLOB_RETENTION.as_secs());
    let tags = blobs.store().tags();
    let mut stream = tags.list_prefix(BLOB_TAG_PREFIX.as_bytes()).await?;
    let mut expired = Vec::new();
    while let Some(info) = stream.next().await {
        let info = info?;
        if tag_fetch_secs(info.name.as_ref()).is_some_and(|secs| secs < cutoff) {
            expired.push(info.name);
        }
    }
    for name in expired {
        tags.delete(name).await?;
    }
    Ok(())
}

/// Parse the embedded fetch time (unix seconds) from a `mailbox/<secs>/<hash>` tag.
fn tag_fetch_secs(name: &[u8]) -> Option<u64> {
    let name = std::str::from_utf8(name).ok()?;
    name.strip_prefix(BLOB_TAG_PREFIX)?
        .split('/')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_id_matches_secret_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let expected = key.public();
        let bs = BlobSync::new(key, dir.path().to_path_buf(), None)
            .await
            .unwrap();
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

    #[test]
    fn tag_fetch_secs_round_trips_protect_blob_format() {
        let secs = 1_700_000_000u64;
        let name = format!("{BLOB_TAG_PREFIX}{secs:020}/{}", hash(7));
        assert_eq!(tag_fetch_secs(name.as_bytes()), Some(secs));
        assert_eq!(tag_fetch_secs(b"other/123/abc"), None);
    }

    #[tokio::test]
    async fn pool_remove_drops_the_hash() {
        let pool = BlobFetchPool::default();
        pool.add_source(hash(1), endpoint_id(1)).await;
        pool.remove(hash(1)).await;
        assert!(pool.is_empty().await);
    }
}
