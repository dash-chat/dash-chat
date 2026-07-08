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
/// Default grace period the mailbox waits before dialing the source for a hash
/// announced with an expected upload, giving a concurrent inline upload of the
/// same blob time to land first. Its purpose is to keep the mailbox from
/// fetching too soon, not to fetch sooner than the next poll. Overridable per
/// server via [`BlobSync::with_upload_grace`] (tests use a much shorter window).
pub const DEFAULT_UPLOAD_GRACE: Duration = Duration::from_secs(60);

#[derive(Default)]
struct PoolEntry {
    sources: BTreeSet<iroh::EndpointId>,
    failures: u32,
    /// Earliest time this entry may be fetched, or `None` to fetch immediately.
    /// Set when a hash is announced, to give a concurrent inline upload time to
    /// arrive before the mailbox dials the source.
    not_before: Option<tokio::time::Instant>,
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    entries: Arc<Mutex<BTreeMap<iroh_blobs::Hash, PoolEntry>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId) {
        self.add_source_inner(hash, source, None).await;
    }

    /// Like [`add_source`](Self::add_source) but holds off fetching a *newly*
    /// added hash until `delay` has passed, so a concurrent inline upload of the
    /// same blob can land first. An already-pending entry keeps its existing
    /// schedule (a re-announce never pushes a fetch further out).
    pub(crate) async fn add_source_after(
        &self,
        hash: iroh_blobs::Hash,
        source: iroh::EndpointId,
        delay: Duration,
    ) {
        self.add_source_inner(hash, source, Some(tokio::time::Instant::now() + delay))
            .await;
        // A deferred entry keeps the pool non-empty, so the fetch loop sleeps a
        // full `pass_interval` and never wakes on its own when the grace expires.
        // Nudge it at the grace boundary so the backstop fetch isn't delayed to
        // the next pass.
        let added = self.added.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            added.notify_one();
        });
    }

    async fn add_source_inner(
        &self,
        hash: iroh_blobs::Hash,
        source: iroh::EndpointId,
        not_before: Option<tokio::time::Instant>,
    ) {
        let mut map = self.entries.lock().await;
        if !map.contains_key(&hash) && map.len() >= MAX_POOL_SIZE {
            // Drop the first (lexicographically earliest) entry to stay within the cap.
            if let Some(oldest) = map.keys().next().copied() {
                map.remove(&oldest);
            }
        }
        let entry = map.entry(hash).or_insert_with(|| PoolEntry {
            not_before,
            ..Default::default()
        });
        entry.sources.insert(source);
        self.added.notify_one();
    }

    pub(crate) async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }

    pub(crate) async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)> {
        let now = tokio::time::Instant::now();
        let map = self.entries.lock().await;
        map.iter()
            .find(|(hash, entry)| {
                !tried.contains(*hash) && entry.not_before.is_none_or(|t| t <= now)
            })
            .map(|(hash, entry)| (*hash, entry.sources.iter().copied().collect()))
    }

    pub(crate) async fn remove(&self, hash: iroh_blobs::Hash) {
        self.entries.lock().await.remove(&hash);
    }

    /// Record that a client uploaded `hash`, then push out the grace window for
    /// that sender's other still-deferred fetches. Removes `hash` (the mailbox
    /// now holds it) and, for every remaining entry that shares a source with it
    /// and whose `not_before` is still in the future, resets `not_before` to a
    /// fresh `grace` from now — steady upload progress keeps the mailbox from
    /// racing the client into a duplicate fetch of a sibling in the same batch.
    /// Entries whose grace has already elapsed are never touched.
    pub(crate) async fn note_upload(&self, hash: iroh_blobs::Hash, grace: Duration) {
        let now = tokio::time::Instant::now();
        let deadline = now + grace;
        let mut bumped = false;
        {
            let mut map = self.entries.lock().await;
            let Some(uploaded) = map.remove(&hash) else {
                return;
            };
            let senders = uploaded.sources;
            for entry in map.values_mut() {
                let deferred = entry.not_before.is_some_and(|t| t > now);
                if deferred && !entry.sources.is_disjoint(&senders) {
                    entry.not_before = Some(deadline);
                    bumped = true;
                }
            }
        }
        // A bumped entry sits past any nudge an earlier announce scheduled, so
        // wake the fetch loop at the new boundary (see `add_source_after`).
        if bumped {
            let added = self.added.clone();
            tokio::spawn(async move {
                tokio::time::sleep(grace).await;
                added.notify_one();
            });
        }
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
    /// Grace window applied to a hash announced with an expected upload before the
    /// mailbox dials its source. Defaults to [`DEFAULT_UPLOAD_GRACE`].
    upload_grace: Duration,
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
        dashchat_utils::endpoint::wait_endpoint_online(
            has_relay,
            &endpoint,
            Duration::from_secs(10),
        )
        .await?;

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
            upload_grace: DEFAULT_UPLOAD_GRACE,
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
            upload_grace: DEFAULT_UPLOAD_GRACE,
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

    /// Override the grace window applied before dialing the source of a hash
    /// announced with an expected upload. Tests use a short window to avoid
    /// waiting out the production [`DEFAULT_UPLOAD_GRACE`].
    pub fn with_upload_grace(mut self, grace: Duration) -> Self {
        self.upload_grace = grace;
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

    /// Store a blob a client pushed directly (via `/blobs/upload`) and protect it
    /// for the retention window, returning its computed hash. Pushed and fetched
    /// blobs are tagged the same way so GC treats them alike. `add_bytes` streams
    /// the data into the store (although each blob is fully loaded into memory)
    /// and yields a temp tag; we swap that for a retention tag before dropping
    /// it so the blob is never left untagged.
    pub async fn store_pushed_blob(&self, data: bytes::Bytes) -> anyhow::Result<iroh_blobs::Hash> {
        let temp_tag = self.blobs.add_bytes(data).temp_tag().await?;
        let hash = temp_tag.hash();
        self.protect_blob(hash).await;
        drop(temp_tag);
        Ok(hash)
    }

    /// Register `source` as a provider for `hash`. When `expect_upload` is set the
    /// fetch is deferred by this server's `upload_grace` so a concurrent inline
    /// upload of the same blob can land first and make the fetch unnecessary;
    /// otherwise the hash is fetchable immediately (no upload is coming).
    pub(crate) async fn add_fetch_source(
        &self,
        hash: iroh_blobs::Hash,
        source: iroh::EndpointId,
        expect_upload: bool,
    ) {
        if expect_upload {
            self.fetch_pool
                .add_source_after(hash, source, self.upload_grace)
                .await
        } else {
            self.fetch_pool.add_source(hash, source).await
        }
    }

    /// A client just uploaded `hash` via `/blobs/upload`. Drop it from the fetch
    /// pool (the mailbox now holds it) and push out the grace window for the
    /// sender's other still-deferred fetches (see [`BlobFetchPool::note_upload`]).
    pub(crate) async fn note_upload(&self, hash: iroh_blobs::Hash) {
        self.fetch_pool.note_upload(hash, self.upload_grace).await;
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

    #[tokio::test(start_paused = true)]
    async fn upload_pushes_out_grace_for_same_sender_siblings() {
        let pool = BlobFetchPool::default();
        let grace = Duration::from_secs(10);
        let sender = endpoint_id(1);
        let other = endpoint_id(2);

        // Sender S announces A and B (uploads expected); a different sender
        // announces D. All three defer their fetch by the grace window.
        pool.add_source_after(hash(1), sender, grace).await; // A
        pool.add_source_after(hash(2), sender, grace).await; // B
        pool.add_source_after(hash(4), other, grace).await; // D

        // Halfway through the window, S's upload of A lands.
        tokio::time::advance(grace / 2).await;
        pool.note_upload(hash(1), grace).await;

        // Just past the ORIGINAL deadline: D (different sender) is fetchable, but
        // B was pushed out to a fresh grace and is still deferred.
        tokio::time::advance(grace / 2 + Duration::from_secs(1)).await;
        let (h, _) = pool.next_untried(&HashSet::new()).await.unwrap();
        assert_eq!(
            h,
            hash(4),
            "different-sender entry keeps its original deadline"
        );
        pool.remove(hash(4)).await;
        assert!(
            pool.next_untried(&HashSet::new()).await.is_none(),
            "same-sender sibling should still be deferred after the bump"
        );

        // Past the bumped deadline, B finally becomes fetchable.
        tokio::time::advance(grace).await;
        let (h, _) = pool.next_untried(&HashSet::new()).await.unwrap();
        assert_eq!(h, hash(2));
    }

    #[tokio::test(start_paused = true)]
    async fn upload_does_not_revive_already_elapsed_grace() {
        let pool = BlobFetchPool::default();
        let grace = Duration::from_secs(10);
        let sender = endpoint_id(1);

        pool.add_source_after(hash(1), sender, grace).await; // A, will be uploaded
        pool.add_source_after(hash(2), sender, grace).await; // B, grace elapses

        // Let the whole window elapse so B is already fetchable.
        tokio::time::advance(grace + Duration::from_secs(1)).await;
        assert!(pool.next_untried(&HashSet::new()).await.is_some());

        // A late upload of A must not push B's already-elapsed deadline back out.
        pool.note_upload(hash(1), grace).await;
        let (h, _) = pool.next_untried(&HashSet::new()).await.unwrap();
        assert_eq!(
            h,
            hash(2),
            "elapsed entry must remain immediately fetchable"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn deferred_entry_is_fetched_at_grace_boundary_not_next_pass() {
        let pool = BlobFetchPool::default();
        pool.add_source_after(hash(1), endpoint_id(1), Duration::from_secs(5))
            .await;
        let start = tokio::time::Instant::now();
        let fetched_at: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));
        let handle = {
            let fetched_at = fetched_at.clone();
            let config = FetchConfig {
                concurrency: 1,
                attempt_timeout: Duration::from_secs(5),
                pass_interval: Duration::from_secs(60),
                retry_cooldown: Duration::from_secs(30),
            };
            tokio::spawn(fetch_loop(pool.clone(), config, move |_item, _t| {
                let fetched_at = fetched_at.clone();
                async move {
                    *fetched_at.lock().await = Some(start.elapsed());
                    true
                }
            }))
        };
        tokio::time::sleep(Duration::from_secs(10)).await;
        let at = fetched_at
            .lock()
            .await
            .expect("deferred entry should have been fetched");
        assert!(
            at >= Duration::from_secs(5) && at < Duration::from_secs(30),
            "expected fetch at the grace boundary (~5s), got {at:?}"
        );
        handle.abort();
    }
}
