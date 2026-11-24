//! Manages syncing blobs referenced in logs over iroh-blobs

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify, watch},
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use aliased::Aliasing;
use dashchat_utils::blob_sync::{BlobDownloader, DownloadProgress, download_capped};
use derive_more::derive::Constructor;
use futures::Stream;
use iroh_blobs::api::downloader::Downloader;
use iroh_blobs::provider::events::{
    AbortReason, ConnectMode, EventMask, EventSender, ProviderMessage, RequestMode, RequestUpdate,
};
use mailboxes_manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use p2panda_store::SqliteStore;

use dashchat_utils::FetchPool;

pub use dashchat_utils::FetchConfig as BlobFetchConfig;

use crate::{
    AsBody, ChatPayload, DeviceId, Payload, TopicId, mailbox::MailboxOperation, stores::OpStore,
};

/// Drop a pending fetch entry after this many consecutive failed passes, so a
/// permanently-unfetchable blob doesn't accumulate steady-state background work.
const MAX_FETCH_FAILURES: u32 = 10;

/// Placeholder used as the operation hash in blob tags when the real op hash is
/// not yet known (e.g. media stored before its enclosing operation is created).
/// Replaced by [`BlobSync::retag_blob`] once the real hash is available.
/// On deletion, the sentinel tag is also attempted so crash-orphaned tags are GC'd.
pub const SENTINEL_OP_HASH: p2panda::Hash = p2panda::Hash::from_bytes([111u8; 32]);

/// Manages syncing blobs referenced in logs over iroh-blobs
#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
    pub sources: MixedSourceLookup,
    downloader: Arc<dyn BlobDownloader>,
}

impl BlobSync {
    pub async fn new(
        endpoint: p2panda::Endpoint,
        root: PathBuf,
        blob_fetch: BlobFetchPool,
        sources: MixedSourceLookup,
        unfetched_tracker: std::sync::Arc<dyn mailbox_client::UnfetchedBlobTracker>,
        local_hub_registry: dashchat_utils::local_hub::LocalHubRegistry,
    ) -> anyhow::Result<Self> {
        let store = iroh_blobs::store::fs::FsStore::load(root).await?;

        let mask = EventMask {
            connected: ConnectMode::Notify,
            get: RequestMode::NotifyLog,
            // Every push waits on the event listener's authorization: accepted
            // only while this node's endpoint hosts a local hub (the only
            // legitimate target of a push), refused and logged otherwise. An
            // always-open push port would let any peer that learns this node's
            // EndpointId write unbounded, never-collected bytes into this
            // store — no GC is configured for it.
            push: RequestMode::Intercept,
            ..EventMask::DEFAULT
        };
        let (events, rx) = EventSender::channel(256, mask);
        let iroh_endpoint = endpoint.endpoint().await?;
        spawn_download_event_listener(
            rx,
            unfetched_tracker,
            iroh_endpoint.id(),
            local_hub_registry,
        );

        let blobs = iroh_blobs::BlobsProtocol::new(&store, Some(events));
        let mixed_alpn =
            p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, endpoint.network_id());
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;
        let downloader = Downloader::builder(&store, &iroh_endpoint)
            .alpn(mixed_alpn.as_slice())
            .build();
        let downloader: Arc<dyn BlobDownloader> = Arc::new(downloader);

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
            downloader,
        })
    }

    /// Clone the downloader so an in-process mailbox can fetch blobs into this
    /// node's shared blob store.
    pub fn downloader(&self) -> Arc<dyn BlobDownloader> {
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

    /// Install a test-only downloader so unit tests can inject deterministic
    /// download behavior (e.g. a never-completing stream) without real iroh.
    #[cfg(test)]
    pub fn set_downloader(&mut self, downloader: Arc<dyn BlobDownloader>) {
        self.downloader = downloader;
    }

    /// Spawn a per-topic fetch task and return a handle that can be polled and
    /// aborted while the download is in flight.
    async fn spawn_fetch(
        &self,
        topic: TopicId,
        hash: iroh_blobs::Hash,
        attempt_timeout: Duration,
    ) -> BlobFetchTask {
        let sources = match self.sources.sources(topic).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::warn!(%hash, ?err, "blob source lookup failed");
                return BlobFetchTask::failed();
            }
        };

        if sources.is_empty() {
            tracing::debug!(%hash, "no blob sources");
            return BlobFetchTask::failed();
        }

        let source_count = sources.len();
        let this = self.clone();
        let (progress_tx, progress_rx) = watch::channel(DownloadProgress::default());
        let handle = tokio::spawn(async move {
            let fetched = download_capped(
                &*this.downloader,
                hash,
                sources,
                attempt_timeout,
                &this.blobs,
                progress_tx,
            )
            .await;
            tracing::debug!(%hash, source_count, fetched, "blob fetch attempt");
            fetched
        });

        BlobFetchTask {
            handle,
            progress: progress_rx,
            topic,
            hash,
        }
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

        let task = self.spawn_fetch(topic, hash, attempt_timeout).await;
        task.await_complete().await
    }

    pub async fn add_to_fetch_pool(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        blob_hash: iroh_blobs::Hash,
    ) -> anyhow::Result<()> {
        // Protect the blob with the tag before fetching.
        // This is the right moment to do it, because if multiple authors
        // publish the same blob, we want tags from each of them
        let tag_name = blob_tag_name(topic, author, operation_hash, blob_hash);
        self.blobs.store().tags().set(tag_name, blob_hash).await?;
        self.fetch_pool.add(topic, blob_hash).await;
        tracing::debug!(hash = %blob_hash, "queued blob for fetch");
        Ok(())
    }

    /// Store blob bytes and tag them with a name that encodes `(topic, author, hash)`
    /// so deletion can be scoped to a specific topic.
    pub async fn store_blob(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        data: impl Into<bytes::Bytes>,
    ) -> anyhow::Result<iroh_blobs::Hash> {
        let tt = self.blobs.blobs().add_bytes(data).temp_tag().await?;
        let hash = tt.hash();
        let tag_name = blob_tag_name(topic, author, operation_hash, hash);
        self.blobs
            .store()
            .tags()
            .set(tag_name, tt.hash_and_format())
            .await?;
        Ok(hash)
    }

    /// Replace the sentinel-tagged entry for a blob with its real operation hash tag.
    /// Called immediately after the enclosing operation is created.
    pub async fn retag_blob(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        blob_hash: iroh_blobs::Hash,
    ) -> anyhow::Result<()> {
        let tags = self.blobs.store().tags();
        let real_tag = blob_tag_name(topic, author, operation_hash, blob_hash);
        tags.set(real_tag, blob_hash).await?;
        let sentinel_tag = blob_tag_name(topic, author, SENTINEL_OP_HASH, blob_hash);
        tags.delete(sentinel_tag).await?;
        Ok(())
    }

    /// Delete all tags for the given `(topic, author, operation_hash, blob_hash)` tuples,
    /// allowing iroh's GC to reclaim data no longer referenced.
    ///
    /// Pass `also_delete_sentinel: true` when the author is self, to clean up any
    /// sentinel-tagged entry left by a crash between `store_blob` and `retag_blob`.
    pub async fn delete_blobs(
        &self,
        topic: TopicId,
        author: DeviceId,
        operation_hash: p2panda::Hash,
        blob_hashes: impl IntoIterator<Item = iroh_blobs::Hash>,
        also_delete_sentinel: bool,
    ) {
        let tags = self.blobs.store().tags();
        for hash in blob_hashes {
            let tag_name = blob_tag_name(topic, author, operation_hash, hash);
            if let Err(err) = tags.delete(tag_name).await {
                tracing::warn!(?err, "failed to delete blob tag");
            }
            if also_delete_sentinel {
                let sentinel_tag = blob_tag_name(topic, author, SENTINEL_OP_HASH, hash);
                if let Err(err) = tags.delete(sentinel_tag).await {
                    tracing::warn!(?err, "failed to delete sentinel blob tag");
                }
            }
            self.fetch_pool.remove(topic, hash).await;
        }
    }

    /// How long a provider can be active without making forward progress before the
    /// fetcher considers it stalled and may pivot to another source.
    const STALL_TIMEOUT: Duration = Duration::from_secs(2);
    /// How long to give a newly-selected provider before treating a lack of
    /// progress as a stall.
    const PROVIDER_WARMUP: Duration = Duration::from_millis(500);

    /// Whether the active provider reported by iroh is a peer (not a mailbox
    /// and not unknown). We only switch away from a download when we are sure
    /// it is coming from a peer; otherwise we risk aborting an already-running
    /// mailbox transfer.
    async fn current_provider_is_peer(&self, progress: &watch::Receiver<DownloadProgress>) -> bool {
        let Some(id) = progress.borrow().provider else {
            return false;
        };
        !self.sources.is_mailbox(id).await
    }

    /// Whether the peer download is making forward progress. A provider is
    /// stalled when it has had the chance to warm up and has not delivered any
    /// bytes for a while.
    fn peer_is_stalled(progress: &DownloadProgress) -> bool {
        let provider_warm = progress
            .provider_changed_at
            .map(|t| t.elapsed() >= Self::PROVIDER_WARMUP)
            .unwrap_or(false);
        let no_recent_progress = progress
            .last_progress_at
            .map(|t| t.elapsed() >= Self::STALL_TIMEOUT)
            .unwrap_or(true);
        provider_warm && no_recent_progress
    }

    /// Whether the peer download has transferred enough of the blob that it is
    /// better to let it finish than to restart from a mailbox. The total size
    /// comes from the local store's view of the blob, so this only returns
    /// true once the store knows the expected size.
    fn peer_is_nearly_done(
        progress: &DownloadProgress,
        status: iroh_blobs::api::proto::BlobStatus,
    ) -> bool {
        let total_size = match status {
            iroh_blobs::api::proto::BlobStatus::Complete { size } => size,
            iroh_blobs::api::proto::BlobStatus::Partial { size: Some(size) } => size,
            _ => return false,
        };
        if total_size == 0 {
            return false;
        }
        progress.bytes_transferred as f64 / total_size as f64 >= 0.7
    }

    /// Whether it makes sense to abort the current in-topic download and restart
    /// it from a mailbox. True only when the active provider is a peer, the
    /// transfer is not already near completion, the transfer is stalled or has
    /// not yet made progress, and at least one registered mailbox reports
    /// already holding the blob.
    async fn can_switch_to_mailbox(
        &self,
        topic: TopicId,
        hash: iroh_blobs::Hash,
        progress: &watch::Receiver<DownloadProgress>,
    ) -> bool {
        if !self.current_provider_is_peer(progress).await {
            return false;
        }
        let progress = progress.borrow().clone();
        let status = self
            .blobs
            .status(hash)
            .await
            .unwrap_or(iroh_blobs::api::proto::BlobStatus::NotFound);
        if Self::peer_is_nearly_done(&progress, status) {
            return false;
        }
        if !Self::peer_is_stalled(&progress) {
            return false;
        }
        let exclude = progress.recently_failed(std::time::Instant::now());
        self.sources
            .any_mailbox_has_blob(topic, hash, &exclude)
            .await
            .unwrap_or(false)
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
        let mut switched_to_mailbox = false;
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
                let task = self.spawn_fetch(topic, hash, remaining).await;
                let mut handle = task.handle;
                let progress = task.progress;
                let poll_interval = Duration::from_millis(250);
                loop {
                    tokio::select! {
                        result = &mut handle => {
                            match result {
                                Ok(true) => return true,
                                Ok(false) => break,
                                Err(join_err) if join_err.is_cancelled() => break,
                                Err(join_err) => {
                                    tracing::warn!(%hash, ?join_err, "fetch task panicked");
                                    break;
                                }
                            }
                        }
                        _ = tokio::time::sleep(poll_interval) => {
                            if self.blobs.has(hash).await.unwrap_or(false) {
                                handle.abort();
                                return true;
                            }

                            if !switched_to_mailbox
                                && self.can_switch_to_mailbox(topic, hash, &progress).await
                            {
                                tracing::debug!(
                                    %hash,
                                    current_provider = ?progress.borrow().provider,
                                    "peer download in progress and mailbox reports blob, switching to mailbox"
                                );
                                switched_to_mailbox = true;
                                handle.abort();
                                break;
                            }
                            if std::time::Instant::now() >= deadline {
                                handle.abort();
                                return false;
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

/// Ongoing per-topic blob download that exposes its current provider and can
/// be aborted.
#[allow(dead_code)]
struct BlobFetchTask {
    handle: JoinHandle<bool>,
    progress: watch::Receiver<DownloadProgress>,
    topic: TopicId,
    hash: iroh_blobs::Hash,
}

impl BlobFetchTask {
    /// A task that has already failed without spawning a download.
    fn failed() -> Self {
        let (_tx, progress) = watch::channel(DownloadProgress::default());
        Self {
            handle: tokio::spawn(std::future::ready(false)),
            progress,
            topic: TopicId::random(),
            hash: iroh_blobs::Hash::new([0u8; 32]),
        }
    }

    /// Abort the in-flight download.
    #[allow(dead_code)]
    fn abort(&self) {
        self.handle.abort_handle().abort();
    }

    /// Wait for the task to finish on its own.
    async fn await_complete(self) -> bool {
        match self.handle.await {
            Ok(result) => result,
            Err(join_err) if join_err.is_cancelled() => false,
            Err(join_err) => {
                tracing::warn!(?join_err, "fetch task panicked");
                false
            }
        }
    }
}

/// Consume provider events on this node's blob endpoint. When a remote endpoint
/// finishes downloading a blob from us, drop the matching `unfetched_blob_hashes`
/// row for that mailbox — the mailbox now has the blob.
fn spawn_download_event_listener(
    mut rx: tokio::sync::mpsc::Receiver<ProviderMessage>,
    tracker: std::sync::Arc<dyn mailbox_client::UnfetchedBlobTracker>,
    own_endpoint_id: iroh::EndpointId,
    local_hub_registry: dashchat_utils::local_hub::LocalHubRegistry,
) {
    tokio::spawn(async move {
        let mut connections: HashMap<u64, iroh::EndpointId> = HashMap::new();
        while let Some(msg) = rx.recv().await {
            match msg {
                ProviderMessage::PushRequestReceived(msg) => {
                    let allowed = local_hub_registry.hosts(own_endpoint_id);
                    if !allowed {
                        tracing::info!(
                            hash = %msg.inner.request.hash,
                            "refusing blob push: this node is not hosting a local hub"
                        );
                    }
                    let verdict = if allowed {
                        Ok(())
                    } else {
                        Err(AbortReason::Permission)
                    };
                    msg.tx.send(verdict).await.ok();
                }
                ProviderMessage::ClientConnectedNotify(msg) => {
                    if let Some(id) = msg.inner.endpoint_id {
                        connections.insert(msg.inner.connection_id, id);
                    }
                }
                ProviderMessage::ConnectionClosed(msg) => {
                    connections.remove(&msg.inner.connection_id);
                }
                ProviderMessage::GetRequestReceivedNotify(msg) => {
                    let hash = msg.inner.request.hash;
                    let endpoint = connections.get(&msg.inner.connection_id).copied();
                    let tracker = tracker.clone();
                    let mut updates = msg.rx;
                    tokio::spawn(async move {
                        while let Ok(Some(update)) = updates.recv().await {
                            if let RequestUpdate::Completed(_) = update {
                                if let Some(endpoint) = endpoint {
                                    let mailbox_id = mailbox_server::encode_mailbox_id(endpoint);
                                    if let Err(err) = tracker.remove(&mailbox_id, &[hash]).await {
                                        tracing::error!(?err, %hash, "failed to clear unfetched blob after download");
                                    }
                                }
                            }
                        }
                    });
                }
                _ => {}
            }
        }
    });
}

fn blob_tag_name(
    topic: TopicId,
    author: DeviceId,
    operation_hash: p2panda::Hash,
    blob_hash: iroh_blobs::Hash,
) -> Vec<u8> {
    let mut name = Vec::with_capacity(128);
    name.extend_from_slice(topic.as_bytes());
    name.extend_from_slice(author.as_bytes());
    name.extend_from_slice(operation_hash.as_bytes());
    name.extend_from_slice(blob_hash.as_bytes());
    name
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(TopicId, iroh_blobs::Hash)>>>,
    /// Consecutive failed-pass count per hash. A hash that can never be fetched
    /// (sender deleted it, provider permanently gone, garbage hash) is evicted
    /// after [`MAX_FETCH_FAILURES`] so it isn't re-attempted every pass forever.
    failures: Arc<Mutex<HashMap<iroh_blobs::Hash, u32>>>,
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
        self.failures.lock().await.remove(&item.1);
    }
    async fn on_failure(&self, item: &Self::Item) {
        let hash = item.1;
        let mut failures = self.failures.lock().await;
        let count = failures.entry(hash).or_insert(0);
        *count += 1;
        if *count >= MAX_FETCH_FAILURES {
            failures.remove(&hash);
            drop(failures);
            self.stack.lock().await.retain(|(_, h)| *h != hash);
            tracing::debug!(%hash, "evicting blob from fetch pool after too many failures");
        }
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}

impl BlobFetchPool {
    // Not pub so that we call it from BlobSync and add a tag at the same time.
    async fn add(&self, topic: TopicId, hash: iroh_blobs::Hash) {
        // A fresh reference resets the failure count, giving an on-demand
        // `load_blob` (or a new message) another full round of attempts.
        self.failures.lock().await.remove(&hash);
        self.stack.lock().await.push((topic, hash));
        self.added.notify_one();
    }

    // Not pub so that we call it from BlobSync and remove a tag at the same time.
    async fn remove(&self, topic: TopicId, hash: iroh_blobs::Hash) {
        self.stack
            .lock()
            .await
            .retain(|(t, h)| *t != topic || *h != hash);
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
    ///
    /// TODO: find a way to avoid re-streaming all operations on startup
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
        topic_store: SqliteStore,
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
                        let Some(topic) = crate::topic::resolve_application_topic(
                            &topic_store,
                            &op.header.verifying_key,
                            &op.header.extensions.log_id(),
                        )
                        .await?
                        else {
                            tracing::error!(
                                author = ?op.header.verifying_key.aliased(),
                                log_id = ?op.header.extensions.log_id().aliased(),
                                "failed to resolve topic for operation",
                            );
                            continue;
                        };
                        for item in media {
                            s.push((topic, item.hash()));
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
    /// Return a list of providers for the blobs in a topic.
    /// Mailboxes come first, then peer subscribers.
    pub async fn sources(&self, topic: TopicId) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let log_id = LogId::from_topic(topic);
        let mut sources = vec![];
        sources.extend(self.mailboxes.get_sources(&topic).await?);
        sources.extend(
            self.op_store
                .get_authors(log_id)
                .await?
                .into_iter()
                .map(|author| iroh::EndpointId::from_bytes(author.as_bytes()))
                .collect::<Result<Vec<iroh::EndpointId>, _>>()?,
        );
        // Never dial ourselves (we already early-return when the blob is local),
        // and dedupe so a provider isn't dialed twice — redundant dials churn
        // iroh connection paths.
        let mut seen = HashSet::new();
        sources.retain(|id| *id != self.self_endpoint && seen.insert(*id));
        Ok(sources)
    }

    /// Whether `endpoint` identifies one of the registered mailboxes.
    pub async fn is_mailbox(&self, endpoint: iroh::EndpointId) -> bool {
        self.mailboxes.is_mailbox(endpoint).await
    }

    /// Probe registered mailboxes for this topic and return whether any report
    /// already holding the blob. Only mailboxes that are tracking `topic`, are
    /// in `Active` health, and are not in `exclude` are considered.
    pub async fn any_mailbox_has_blob(
        &self,
        topic: TopicId,
        hash: iroh_blobs::Hash,
        exclude: &[iroh::EndpointId],
    ) -> anyhow::Result<bool> {
        // Only consider mailboxes that are subscribed to this topic. Otherwise
        // the switch could fire against a mailbox that won't appear in the
        // restarted provider list at all.
        let topic_sources = self.sources(topic).await?;
        let exclude_set: HashSet<iroh::EndpointId> = exclude.iter().copied().collect();
        for decoded in topic_sources {
            if !self.is_mailbox(decoded).await || exclude_set.contains(&decoded) {
                continue;
            }
            let mailbox_id: mailbox_client::MailboxId =
                mailbox_server::encode_mailbox_id(decoded).into();
            let Some(tracked) = self.mailboxes.tracked_mailbox(&mailbox_id).await else {
                continue;
            };
            if tracked.health().borrow().status != mailbox_client::SyncStatus::Active {
                continue;
            }
            match tracked.client().await.has_blob(hash).await {
                Ok(true) => {
                    tracing::debug!(%hash, %mailbox_id, "mailbox reports blob available");
                    return Ok(true);
                }
                Ok(false) => {}
                Err(err) => {
                    tracing::debug!(%hash, %mailbox_id, ?err, "mailbox blob probe failed");
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }

    fn test_endpoint(n: u8) -> iroh::EndpointId {
        let mut seed = [0u8; 32];
        seed[0] = n;
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        iroh::EndpointId::from_bytes(signing_key.verifying_key().as_bytes())
            .expect("valid test key")
    }

    /// Every download attempt is slow and never completes after it emits a
    /// single `TryProvider` event for the given endpoint. The number of calls
    /// recorded here lets tests verify whether an attempt was cancelled and
    /// restarted.
    struct SlowDownloader {
        calls: std::sync::atomic::AtomicUsize,
        provider: iroh::EndpointId,
    }

    #[async_trait::async_trait]
    impl dashchat_utils::blob_sync::BlobDownloader for SlowDownloader {
        async fn download_with_opts(
            &self,
            _options: iroh_blobs::api::downloader::DownloadRequest,
        ) -> anyhow::Result<dashchat_utils::blob_sync::BoxedDownloadStream> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let provider = self.provider;
            let stream = async_stream::stream! {
                yield iroh_blobs::api::downloader::DownloadProgressItem::TryProvider {
                    id: provider,
                    request: std::sync::Arc::new(iroh_blobs::protocol::GetRequest::blob(
                        iroh_blobs::Hash::new([0; 32]),
                    )),
                };
                std::future::pending::<()>().await;
            };
            Ok(Box::pin(stream))
        }
    }

    /// A fake mailbox that reports the blob as stored once it has been queried
    /// enough times.
    struct FakeMailbox {
        id: mailbox_client::MailboxId,
        target_hash: iroh_blobs::Hash,
        queried: std::sync::atomic::AtomicUsize,
        report_available_after_queries: usize,
    }

    #[async_trait::async_trait]
    impl mailbox_client::MailboxClient<MailboxOperation> for FakeMailbox {
        fn id(&self) -> mailbox_client::MailboxId {
            self.id.clone()
        }

        async fn publish(
            &self,
            _ops: Vec<MailboxOperation>,
        ) -> Result<mailbox_client::PublishResponse<MailboxOperation>, anyhow::Error> {
            Ok(mailbox_client::PublishResponse::default())
        }

        async fn fetch(
            &self,
            _request: mailbox_client::FetchRequest<MailboxOperation>,
        ) -> Result<mailbox_client::FetchResponse<MailboxOperation>, anyhow::Error> {
            Ok(mailbox_client::FetchResponse(
                std::collections::BTreeMap::new(),
            ))
        }

        async fn has_blob(&self, hash: iroh_blobs::Hash) -> Result<bool, anyhow::Error> {
            let count = self
                .queried
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(count >= self.report_available_after_queries && hash == self.target_hash)
        }
    }

    /// Collect all persistent tag names from a blob store.
    async fn list_tag_names(blobs: &iroh_blobs::BlobsProtocol) -> Vec<Vec<u8>> {
        use tokio_stream::StreamExt;
        let stream = blobs.store().tags().list().await.unwrap();
        tokio::pin!(stream);
        let mut names = vec![];
        while let Some(Ok(info)) = stream.next().await {
            names.push(info.name.0.to_vec());
        }
        names
    }

    async fn tag_count_for_hash(
        blobs: &iroh_blobs::BlobsProtocol,
        hash: iroh_blobs::Hash,
    ) -> usize {
        list_tag_names(blobs)
            .await
            .into_iter()
            .filter(|name| name.ends_with(hash.as_bytes()))
            .count()
    }

    #[cfg(feature = "testing")]
    mod integration {
        use super::*;
        use crate::{NodeConfig, testing::TestNode, topic::TopicId};

        #[tokio::test(flavor = "multi_thread")]
        async fn two_authors_same_blob_get_distinct_tags() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let operation_hash = p2panda::Hash::digest(b"test-op");
            let data = b"shared-media-blob";

            let hash_alice = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), operation_hash, data.as_ref())
                .await
                .unwrap();
            let hash_bobbi = bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), operation_hash, data.as_ref())
                .await
                .unwrap();

            // Same content → same hash.
            assert_eq!(hash_alice, hash_bobbi);

            // Each node has exactly one tag for that hash (their own authorship tag).
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash_alice).await, 1);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash_bobbi).await, 1);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn deleting_alice_author_tag_leaves_one_tag_each() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let op_hash = p2panda::Hash::digest(b"test-op");
            let data = b"shared-media-blob";

            // Both nodes store the same blob under their own authorship.
            let hash = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            alice
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, alice.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();

            // Before deletion: two tags each.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 2);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 2);

            // Delete alice's authorship tag on both nodes.
            alice
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), op_hash, [hash], false)
                .await;
            bobbi
                .blob_sync()
                .delete_blobs(topic, alice.device_id(), op_hash, [hash], false)
                .await;

            // One tag remains on each node (bobbi's).
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 1);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 1);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn deleting_both_author_tags_leaves_no_tags() {
            let alice = TestNode::new(NodeConfig::testing(), "alice").await;
            let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
            let topic = TopicId::random();
            let op_hash = p2panda::Hash::digest(b"test-op");
            let data = b"gc-target-blob";

            let hash = alice
                .blob_sync()
                .store_blob(topic, alice.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            alice
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, alice.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();
            bobbi
                .blob_sync()
                .store_blob(topic, bobbi.device_id(), op_hash, data.as_ref())
                .await
                .unwrap();

            // Delete all tags on both nodes.
            for node in [&alice, &bobbi] {
                node.blob_sync()
                    .delete_blobs(topic, alice.device_id(), op_hash, [hash], false)
                    .await;
                node.blob_sync()
                    .delete_blobs(topic, bobbi.device_id(), op_hash, [hash], false)
                    .await;
            }

            // No pinning tags remain — blob is eligible for GC on the next cycle.
            assert_eq!(tag_count_for_hash(&alice.blobs(), hash).await, 0);
            assert_eq!(tag_count_for_hash(&bobbi.blobs(), hash).await, 0);
        }
    }

    #[tokio::test]
    async fn evicts_a_hash_after_max_failures_and_keeps_others() {
        let pool = BlobFetchPool::default();
        let dead = hash(1);
        let live = hash(2);
        pool.add(TopicId::random(), dead).await;
        pool.add(TopicId::random(), live).await;

        for _ in 0..MAX_FETCH_FAILURES {
            pool.on_failure(&(TopicId::random(), dead)).await;
        }

        let remaining = pool.stack.lock().await;
        assert!(remaining.iter().all(|(_, h)| *h == live));
        assert_eq!(remaining.len(), 1);
    }

    #[tokio::test]
    async fn evicts_every_entry_sharing_an_evicted_hash() {
        let pool = BlobFetchPool::default();
        let h = hash(7);
        pool.add(TopicId::random(), h).await;
        pool.add(TopicId::random(), h).await;

        for _ in 0..MAX_FETCH_FAILURES {
            pool.on_failure(&(TopicId::random(), h)).await;
        }
        assert!(pool.is_empty().await);
    }

    #[tokio::test]
    async fn fetch_now_cancels_slow_task_when_blob_becomes_local() {
        use dashchat_utils::FetchConfig as BlobFetchConfig;
        use dashchat_utils::blob_sync::{BlobDownloader, BoxedDownloadStream};
        use std::time::Duration;

        struct NeverEndingDownloader;
        #[async_trait::async_trait]
        impl BlobDownloader for NeverEndingDownloader {
            async fn download_with_opts(
                &self,
                _options: iroh_blobs::api::downloader::DownloadRequest,
            ) -> anyhow::Result<BoxedDownloadStream> {
                Ok(Box::pin(futures::stream::pending()))
            }
        }

        // Slow background loop so it doesn't race with this test.
        let mut config = crate::NodeConfig::testing();
        config.blob_fetch = BlobFetchConfig {
            concurrency: 1,
            attempt_timeout: Duration::from_secs(60),
            pass_interval: Duration::from_secs(3600),
            retry_cooldown: Duration::from_secs(3600),
        };

        let alice = crate::testing::TestNode::new(config.clone(), "alice").await;
        let bobbi = crate::testing::TestNode::new(config, "bobbi").await;
        crate::testing::introduce_peers([&alice, &bobbi])
            .await
            .unwrap();
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = bobbi.direct_chat_with(&alice);

        let content = b"local-inject";
        let hash = iroh_blobs::Hash::new(content.as_slice());

        bobbi
            .blob_sync()
            .add_to_fetch_pool(
                chat.into(),
                alice.device_id(),
                p2panda::Hash::from_bytes([0; 32]),
                hash,
            )
            .await
            .unwrap();

        let mut blob_sync = bobbi.blob_sync().clone();
        blob_sync.set_downloader(Arc::new(NeverEndingDownloader));

        let blob_sync_for_inject = bobbi.blob_sync().clone();
        let content_for_inject = content.to_vec();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            blob_sync_for_inject
                .blobs
                .add_bytes(content_for_inject)
                .await
                .unwrap();
        });

        let result = tokio::time::timeout(
            Duration::from_millis(1500),
            blob_sync.fetch_now(hash, Duration::from_secs(10)),
        )
        .await;

        assert!(
            result.is_ok(),
            "fetch_now should return once the blob is available locally"
        );
        assert!(result.unwrap(), "fetch_now should report success");
    }

    #[tokio::test]
    async fn fetch_now_cancels_slow_task_when_mailbox_gains_blob() {
        use dashchat_utils::FetchConfig as BlobFetchConfig;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        // Slow background loop so it doesn't race with this test.
        let mut config = crate::NodeConfig::testing();
        config.blob_fetch = BlobFetchConfig {
            concurrency: 1,
            attempt_timeout: Duration::from_secs(60),
            pass_interval: Duration::from_secs(3600),
            retry_cooldown: Duration::from_secs(3600),
        };

        let alice = crate::testing::TestNode::new(config.clone(), "alice").await;
        let bobbi = crate::testing::TestNode::new(config, "bobbi").await;
        crate::testing::introduce_peers([&alice, &bobbi])
            .await
            .unwrap();
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = bobbi.direct_chat_with(&alice);

        let content = b"mailboxed-blob";
        let hash = iroh_blobs::Hash::new(content.as_slice());

        bobbi
            .blob_sync()
            .add_to_fetch_pool(
                chat.into(),
                alice.device_id(),
                p2panda::Hash::from_bytes([0; 32]),
                hash,
            )
            .await
            .unwrap();

        // Register a fake mailbox that will claim to have the blob after a
        // couple of probes, triggering cancellation of the slow P2P attempt.
        // Its id must encode a valid endpoint id so it appears in the topic's
        // mailbox source list.
        let mailbox_endpoint = test_endpoint(2);
        let mailbox_id: mailbox_client::MailboxId =
            mailbox_server::encode_mailbox_id(mailbox_endpoint).into();
        bobbi
            .mailboxes
            .register(FakeMailbox {
                id: mailbox_id,
                target_hash: hash,
                queried: AtomicUsize::new(0),
                report_available_after_queries: 2,
            })
            .await;

        let peer = test_endpoint(1);
        let downloader = Arc::new(SlowDownloader {
            calls: AtomicUsize::new(0),
            provider: peer,
        });

        let mut blob_sync = bobbi.blob_sync().clone();
        blob_sync.set_downloader(downloader.clone());

        // The download is never-ending, so fetch_now should time out. The key
        // assertion is that the downloader was called more than once, proving
        // the slow task was cancelled and restarted when the mailbox became
        // available.
        let _ = tokio::time::timeout(
            Duration::from_millis(2000),
            blob_sync.fetch_now(hash, Duration::from_secs(10)),
        )
        .await;

        let calls = downloader.calls.load(Ordering::SeqCst);
        assert!(
            calls >= 2,
            "slow download should have been cancelled and restarted (calls: {calls})"
        );
    }

    #[tokio::test]
    async fn fetch_now_does_not_switch_when_already_downloading_from_mailbox() {
        use dashchat_utils::FetchConfig as BlobFetchConfig;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        // Slow background loop so it doesn't race with this test.
        let mut config = crate::NodeConfig::testing();
        config.blob_fetch = BlobFetchConfig {
            concurrency: 1,
            attempt_timeout: Duration::from_secs(60),
            pass_interval: Duration::from_secs(3600),
            retry_cooldown: Duration::from_secs(3600),
        };

        let alice = crate::testing::TestNode::new(config.clone(), "alice").await;
        let bobbi = crate::testing::TestNode::new(config, "bobbi").await;
        crate::testing::introduce_peers([&alice, &bobbi])
            .await
            .unwrap();
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = bobbi.direct_chat_with(&alice);

        let content = b"mailboxed-blob";
        let hash = iroh_blobs::Hash::new(content.as_slice());

        bobbi
            .blob_sync()
            .add_to_fetch_pool(
                chat.into(),
                alice.device_id(),
                p2panda::Hash::from_bytes([0; 32]),
                hash,
            )
            .await
            .unwrap();

        let mailbox_endpoint = test_endpoint(2);
        let mailbox_id: mailbox_client::MailboxId =
            mailbox_server::encode_mailbox_id(mailbox_endpoint).into();

        // Register a fake mailbox that claims to have the blob immediately. If
        // the switch logic failed to check the current provider, this would
        // abort and restart the download.
        bobbi
            .mailboxes
            .register(FakeMailbox {
                id: mailbox_id,
                target_hash: hash,
                queried: AtomicUsize::new(0),
                report_available_after_queries: 0,
            })
            .await;

        let downloader = Arc::new(SlowDownloader {
            calls: AtomicUsize::new(0),
            provider: mailbox_endpoint,
        });

        let mut blob_sync = bobbi.blob_sync().clone();
        blob_sync.set_downloader(downloader.clone());

        let _ = tokio::time::timeout(
            Duration::from_millis(1000),
            blob_sync.fetch_now(hash, Duration::from_secs(10)),
        )
        .await;

        let calls = downloader.calls.load(Ordering::SeqCst);
        assert_eq!(
            calls, 1,
            "download from a mailbox should not be restarted to switch to a mailbox (calls: {calls})"
        );
    }

    #[tokio::test]
    async fn fetch_now_does_not_switch_to_mailbox_that_just_failed_over_iroh() {
        use dashchat_utils::FetchConfig as BlobFetchConfig;
        use dashchat_utils::blob_sync::{BlobDownloader, BoxedDownloadStream};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        /// A downloader that starts a peer download, reports that the peer failed
        /// over iroh, then emits a `TryProvider` for a mailbox endpoint and stalls.
        /// The mailbox is registered and claims to have the blob. If the switch
        /// logic does not exclude recently-failed providers, it will abort and
        /// restart; if it does, the original peer download keeps running.
        struct PeerThenFailingMailboxDownloader {
            calls: AtomicUsize,
            peer: iroh::EndpointId,
            mailbox: iroh::EndpointId,
        }

        #[async_trait::async_trait]
        impl BlobDownloader for PeerThenFailingMailboxDownloader {
            async fn download_with_opts(
                &self,
                _options: iroh_blobs::api::downloader::DownloadRequest,
            ) -> anyhow::Result<BoxedDownloadStream> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let peer = self.peer;
                let mailbox = self.mailbox;
                let stream = async_stream::stream! {
                    yield iroh_blobs::api::downloader::DownloadProgressItem::TryProvider {
                        id: peer,
                        request: std::sync::Arc::new(iroh_blobs::protocol::GetRequest::blob(
                            iroh_blobs::Hash::new([0; 32]),
                        )),
                    };
                    // Give the poll loop time to see the peer as active.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    // The peer failed over iroh.
                    yield iroh_blobs::api::downloader::DownloadProgressItem::ProviderFailed {
                        id: peer,
                        request: std::sync::Arc::new(iroh_blobs::protocol::GetRequest::blob(
                            iroh_blobs::Hash::new([0; 32]),
                        )),
                    };
                    // Then iroh tries the mailbox next.
                    yield iroh_blobs::api::downloader::DownloadProgressItem::TryProvider {
                        id: mailbox,
                        request: std::sync::Arc::new(iroh_blobs::protocol::GetRequest::blob(
                            iroh_blobs::Hash::new([0; 32]),
                        )),
                    };
                    // The mailbox also fails over iroh.
                    yield iroh_blobs::api::downloader::DownloadProgressItem::ProviderFailed {
                        id: mailbox,
                        request: std::sync::Arc::new(iroh_blobs::protocol::GetRequest::blob(
                            iroh_blobs::Hash::new([0; 32]),
                        )),
                    };
                    // Stall forever; there are no more providers to try.
                    std::future::pending::<()>().await;
                };
                Ok(Box::pin(stream))
            }
        }

        // Slow background loop so it doesn't race with this test.
        let mut config = crate::NodeConfig::testing();
        config.blob_fetch = BlobFetchConfig {
            concurrency: 1,
            attempt_timeout: Duration::from_secs(60),
            pass_interval: Duration::from_secs(3600),
            retry_cooldown: Duration::from_secs(3600),
        };

        let alice = crate::testing::TestNode::new(config.clone(), "alice").await;
        let bobbi = crate::testing::TestNode::new(config, "bobbi").await;
        crate::testing::introduce_peers([&alice, &bobbi])
            .await
            .unwrap();
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = bobbi.direct_chat_with(&alice);

        let content = b"mailboxed-blob";
        let hash = iroh_blobs::Hash::new(content.as_slice());

        bobbi
            .blob_sync()
            .add_to_fetch_pool(
                chat.into(),
                alice.device_id(),
                p2panda::Hash::from_bytes([0; 32]),
                hash,
            )
            .await
            .unwrap();

        let peer = test_endpoint(1);
        let mailbox_endpoint = test_endpoint(2);
        let mailbox_id: mailbox_client::MailboxId =
            mailbox_server::encode_mailbox_id(mailbox_endpoint).into();

        // Register a fake mailbox that claims to have the blob immediately. Its
        // iroh endpoint will be in recent_failures, so it must not trigger a
        // switch even though the HTTP probe says yes.
        bobbi
            .mailboxes
            .register(FakeMailbox {
                id: mailbox_id,
                target_hash: hash,
                queried: AtomicUsize::new(0),
                report_available_after_queries: 0,
            })
            .await;

        let downloader = Arc::new(PeerThenFailingMailboxDownloader {
            calls: AtomicUsize::new(0),
            peer,
            mailbox: mailbox_endpoint,
        });

        let mut blob_sync = bobbi.blob_sync().clone();
        blob_sync.set_downloader(downloader.clone());

        let _ = tokio::time::timeout(
            Duration::from_millis(1500),
            blob_sync.fetch_now(hash, Duration::from_secs(10)),
        )
        .await;

        let calls = downloader.calls.load(Ordering::SeqCst);
        assert_eq!(
            calls, 1,
            "should not switch to a mailbox that just failed over iroh (calls: {calls})"
        );
    }

    #[tokio::test]
    async fn re_adding_resets_the_failure_count() {
        let pool = BlobFetchPool::default();
        let h = hash(3);
        pool.add(TopicId::random(), h).await;

        for _ in 0..(MAX_FETCH_FAILURES - 1) {
            pool.on_failure(&(TopicId::random(), h)).await;
        }
        pool.add(TopicId::random(), h).await;
        // The reset means one more failure must not evict it.
        pool.on_failure(&(TopicId::random(), h)).await;
        assert!(!pool.is_empty().await);
    }
}
