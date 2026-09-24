//! The Dash Router LAN gossip shell, embedded (spec
//! dash-router/docs/superpowers/specs/2026-09-22-dash-chat-lan-router-design.md §4).
//!
//! Everything is a no-op unless BOTH the `lan-router` cargo feature is on
//! and `NodeConfig::enable_lan_router` is true. The node calls this module
//! at four points: `LanRouter::spawn` in `Node::init`, `subscribe_topic`
//! from `initialize_topic`, and, after an operation is acked, `authored`
//! for our own ops (the router pushes them) or `hint_changed` for others'.
//! With the feature off, this file is the stub below and every call site's
//! `Option` is `None`.

use std::path::PathBuf;
use std::sync::Arc;

use p2panda::VerifyingKey;
use p2panda::streams::ProcessedOperation;
use tokio::sync::mpsc;

use crate::node::actor::Command;
use crate::payload::Payload;
use crate::stores::OpStore;
use crate::topic::TopicId;

/// What the node hands the router at startup.
pub struct LanRouterParams {
    pub enabled: bool,
    pub data_path: PathBuf,
    pub device_id: VerifyingKey,
    pub op_store: OpStore,
    pub push_debounce: std::time::Duration,
    pub push_max_latency: std::time::Duration,
    /// Read only by the feature-on router.
    #[cfg_attr(not(feature = "lan-router"), allow(dead_code))]
    pub(crate) actor_tx: mpsc::Sender<Command>,
}

#[cfg(not(feature = "lan-router"))]
pub struct LanRouter;

#[cfg(not(feature = "lan-router"))]
impl LanRouter {
    /// Always `None`: built without the `lan-router` feature.
    pub async fn spawn(params: LanRouterParams) -> anyhow::Result<Option<Arc<Self>>> {
        if params.enabled {
            tracing::warn!("enable_lan_router is set but this build has no lan-router feature");
        }
        Ok(None)
    }
    pub async fn subscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
        Ok(())
    }
    pub async fn unsubscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn hint_changed(&self, _author: VerifyingKey, _topic: TopicId) {}
    pub async fn authored(&self, _operation: &ProcessedOperation<Payload>) -> anyhow::Result<()> {
        Ok(())
    }
    pub async fn shutdown(&self) {}
}

#[cfg(feature = "lan-router")]
pub use imp::LanRouter;

#[cfg(feature = "lan-router")]
mod imp {
    use super::*;

    use std::collections::{BTreeSet, HashMap};
    use std::sync::RwLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use aliased::Aliasing as _;
    use anyhow::{Context as _, anyhow, ensure};
    use dash_router::core::{LogRanges, Op, Ranges, RouterConfig, Seq, Units};
    use dash_router::policy::{IntervalPolicy, PushDebouncePolicy};
    use dash_router::{
        AsyncStorage, CoreConfig, DiskRelayStore, GossipPublisher, GossipSubscription,
        GossipTransport, PeerKey, PolicyIntervals, RouterEvent, RouterHandle, WatchableStorage,
    };
    use futures::StreamExt as _;
    use p2panda::operation::{Header, LogId, Operation};
    use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
    use p2panda_core::Body;
    use rand::SeedableRng as _;
    use serde::{Deserialize, Serialize};
    use serde_bytes::ByteBuf;
    use tokio::sync::{Mutex, broadcast, oneshot};
    use tokio::task::JoinHandle;
    use tokio_stream::wrappers::ReceiverStream;

    use crate::DeviceId;

    /// The router's log identity: `(LogId, author)`, channel first so the
    /// relay store's ordered scans keep one topic's logs contiguous. The
    /// channel (`LogId = blake3(topic)`) is what a subscription names.
    #[derive(
        Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug,
    )]
    pub(crate) struct RouterLog {
        log_id: [u8; 32],
        author: [u8; 32],
    }

    impl RouterLog {
        pub(crate) fn new(log_id: LogId, author: VerifyingKey) -> Self {
            Self {
                log_id: *log_id.as_bytes(),
                author: *author.as_bytes(),
            }
        }

        pub(crate) fn log_id(&self) -> LogId {
            LogId::from(p2panda::Hash::from_bytes(self.log_id))
        }

        pub(crate) fn verifying_key(&self) -> anyhow::Result<VerifyingKey> {
            Ok(VerifyingKey::from_bytes(&self.author)?)
        }
    }

    impl std::fmt::Display for RouterLog {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}/{}",
                hex::encode(&self.log_id[..4]),
                AuthorKey(self.author)
            )
        }
    }

    /// The per-author half of a [`RouterLog`]: raw verifying-key bytes,
    /// kept raw (not a `VerifyingKey`) so an id received off the wire with
    /// an invalid key still round-trips through `Log::new` losslessly.
    #[derive(
        Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug,
    )]
    pub(crate) struct AuthorKey([u8; 32]);

    impl std::fmt::Display for AuthorKey {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", hex::encode(&self.0[..4]))
        }
    }

    impl dash_router::core::Log for RouterLog {
        type Channel = LogId;
        type Author = AuthorKey;
        fn channel(&self) -> LogId {
            self.log_id()
        }
        fn author(&self) -> AuthorKey {
            AuthorKey(self.author)
        }
        fn new(channel: LogId, author: AuthorKey) -> Self {
            Self {
                log_id: *channel.as_bytes(),
                author: author.0,
            }
        }
    }

    impl dash_router::disk::LogKey for RouterLog {
        const WIDTH: usize = 64;
        fn write_key(&self, out: &mut Vec<u8>) {
            out.extend_from_slice(&self.log_id);
            out.extend_from_slice(&self.author);
        }
        fn read_key(bytes: &[u8]) -> Option<Self> {
            let all: [u8; 64] = bytes.get(..64)?.try_into().ok()?;
            Some(Self {
                log_id: all[..32].try_into().ok()?,
                author: all[32..].try_into().ok()?,
            })
        }
    }

    /// The router's ext store: Dash Chat's own `OpStore`, seen as
    /// `(LogId, author)` logs. Only logs under a registered topic exist for
    /// the router, and only acked seqs are held or served (spec §4.2).
    #[derive(Clone)]
    pub(crate) struct OpStoreExt {
        store: OpStore,
        /// `LogId → (TopicId, import channel)`: the inverse of
        /// `LogId::from_topic` for the topics this node follows, paired with
        /// the channel that topic's imported ops go down (consumed by the
        /// actor's `Command::Import`). One map so (un)registration is a
        /// single lock. Read on every held/fetch/ingest; written on
        /// (un)subscribe. Never held across an await.
        topics: Arc<RwLock<HashMap<LogId, (TopicId, mpsc::Sender<Operation>)>>>,
        hints: broadcast::Sender<BTreeSet<RouterLog>>,
        rejected: Arc<AtomicU64>,
        forwarded: Arc<AtomicU64>,
    }

    impl OpStoreExt {
        pub(crate) fn new(store: OpStore) -> Self {
            Self {
                store,
                topics: Default::default(),
                hints: broadcast::channel(64).0,
                rejected: Default::default(),
                forwarded: Default::default(),
            }
        }

        /// Register a topic and the channel its imported ops go down.
        /// Returns false if already registered.
        pub(crate) fn register_topic(
            &self,
            topic: TopicId,
            import_tx: mpsc::Sender<Operation>,
        ) -> bool {
            use std::collections::hash_map::Entry;
            match self.topics.write().unwrap().entry(LogId::from_topic(topic)) {
                Entry::Occupied(_) => false,
                Entry::Vacant(e) => {
                    e.insert((topic, import_tx));
                    true
                }
            }
        }

        pub(crate) fn unregister_topic(&self, topic: TopicId) {
            self.topics
                .write()
                .unwrap()
                .remove(&LogId::from_topic(topic));
        }

        #[cfg(test)]
        pub(crate) fn has_topic(&self, log_id: &LogId) -> bool {
            self.topics.read().unwrap().contains_key(log_id)
        }

        pub(crate) fn hint(&self, log: RouterLog) {
            let _ = self.hints.send(BTreeSet::from([log])); // no receivers is fine
        }

        #[cfg(test)]
        pub(crate) fn rejected(&self) -> u64 {
            self.rejected.load(Ordering::Relaxed)
        }

        /// Ops ingested that were not in the op store yet, handed to p2panda.
        pub(crate) fn forwarded(&self) -> u64 {
            self.forwarded.load(Ordering::Relaxed)
        }

        fn topic_of(&self, log_id: &LogId) -> Option<TopicId> {
            self.topics.read().unwrap().get(log_id).map(|(t, _)| *t)
        }

        fn import_tx_of(&self, log_id: &LogId) -> Option<mpsc::Sender<Operation>> {
            self.topics
                .read()
                .unwrap()
                .get(log_id)
                .map(|(_, tx)| tx.clone())
        }

        /// Acked seqs present for one log, as ranges; empty when nothing is
        /// acked, the topic is unknown, or the author bytes are not a valid
        /// key (a peer can name such a log; it is unknown, not an error).
        async fn held_one(&self, log: &RouterLog) -> anyhow::Result<Ranges> {
            let Some(topic) = self.topic_of(&log.log_id()) else {
                return Ok(Ranges::empty());
            };
            let Ok(author) = log.verifying_key() else {
                return Ok(Ranges::empty());
            };
            let author = DeviceId::from(author);
            let log_id = log.log_id();
            let Some(acked) = self
                .store
                .acked_log_height(&topic, &author, &log_id)
                .await?
            else {
                return Ok(Ranges::empty());
            };
            let seqs = self.store.get_log_seqs(&author, &log_id).await?;
            Ok(Ranges::from_seqs(seqs.into_iter().filter(|q| *q <= acked)))
        }
    }

    impl AsyncStorage<RouterLog> for OpStoreExt {
        async fn held_of(
            &self,
            logs: &BTreeSet<RouterLog>,
        ) -> anyhow::Result<LogRanges<RouterLog>> {
            let mut out = LogRanges::empty();
            for log in logs {
                out.insert(*log, self.held_one(log).await?);
            }
            Ok(out)
        }

        async fn held_all(&self) -> anyhow::Result<LogRanges<RouterLog>> {
            let log_ids: Vec<LogId> = self.topics.read().unwrap().keys().copied().collect();
            let mut out = LogRanges::empty();
            for log_id in log_ids {
                for (author, _height) in self.store.get_log_heights(&log_id).await? {
                    let log = RouterLog::new(log_id, *author);
                    let r = self.held_one(&log).await?;
                    if !r.is_empty() {
                        out.insert(log, r);
                    }
                }
            }
            Ok(out)
        }

        async fn fetch(
            &self,
            ranges: &LogRanges<RouterLog>,
        ) -> anyhow::Result<Vec<(RouterLog, Seq, Op)>> {
            let mut out = Vec::new();
            for (log, r) in ranges.iter() {
                // Nothing wanted: don't read the log (or warn about it).
                if r.is_empty() {
                    continue;
                }
                let Some(topic) = self.topic_of(&log.log_id()) else {
                    continue;
                };
                // A bogus author is an unknown log, not a batch failure.
                let Ok(author) = log.verifying_key() else {
                    continue;
                };
                let author = DeviceId::from(author);
                let log_id = log.log_id();
                let Some(acked) = self
                    .store
                    .acked_log_height(&topic, &author, &log_id)
                    .await?
                else {
                    continue;
                };
                // `get_log`'s cursor is exclusive ("after"): start one below
                // the first wanted seq, or from the beginning when that is 0.
                let after = r.boundaries().first().and_then(|s| s.checked_sub(1));
                for op in self.store.get_log(&author, &log_id, after).await? {
                    let seq = op.header.seq_num;
                    if seq > acked || !r.contains(seq) {
                        continue;
                    }
                    out.push((
                        log,
                        seq,
                        Op {
                            header: op.header.encode(),
                            payload: op.body.as_ref().map(|b| b.to_bytes()),
                        },
                    ));
                }
            }
            Ok(out)
        }

        async fn ingest(&mut self, log: RouterLog, seq: Seq, op: Op) -> anyhow::Result<()> {
            let result: anyhow::Result<()> = async {
                let header = Header::decode(&op.header).context("decoding header")?;
                ensure!(
                    header.seq_num == seq,
                    "header seq {} != {seq}",
                    header.seq_num
                );
                ensure!(
                    header.verifying_key == log.verifying_key()?,
                    "header author != log author"
                );
                ensure!(
                    header.extensions.log_id() == log.log_id(),
                    "header log id != log"
                );
                let hash = header.hash();
                // Held state is acked-only, so the router treats ops p2panda
                // stored but has not acked yet as novel and ingests them again.
                if self.store.has_operation(&hash).await? {
                    tracing::trace!(%log, seq, "lan router ingest: op already stored");
                    return Ok(());
                }
                let tx = self
                    .import_tx_of(&log.log_id())
                    .ok_or_else(|| anyhow!("no topic for log {log}"))?;
                let operation = Operation {
                    hash,
                    header,
                    body: op.payload.as_deref().map(Body::from_bytes),
                };
                // Signature and payload hash/size, as p2panda's ingest checks:
                // p2panda forwards imported ops to live-sync peers before its
                // own pipeline validates them, so a forgery must stop here.
                p2panda_core::validate_operation(&operation).context("invalid operation")?;
                tx.send(operation)
                    .await
                    .map_err(|_| anyhow!("import channel closed"))?;
                self.forwarded.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(%log, seq, "lan router forwarded a new op to p2panda");
                Ok(())
            }
            .await;
            if result.is_err() {
                self.rejected.fetch_add(1, Ordering::Relaxed);
            }
            result
        }
    }

    impl WatchableStorage<RouterLog> for OpStoreExt {
        fn changed(&self) -> broadcast::Receiver<BTreeSet<RouterLog>> {
            self.hints.subscribe()
        }
    }

    /// The well-known router overlay. The topic name comes from
    /// `dash_router::GOSSIP_TOPIC` (not hashed as a local literal) so this
    /// side and dash-router's own `panda.rs` transport can never drift:
    /// dash-router ties that constant to `WIRE_VERSION` with a
    /// compile-time assert.
    ///
    /// The router floods to every member of the node's gossip overlay on
    /// this topic, which is not LAN-scoped by construction: production
    /// nodes share one network id, and the overlay includes peers reached
    /// through bootstrap nodes and the relay. Op bodies are not end-to-end
    /// encrypted, so every overlay member reads them, and relays keep them
    /// in `lan_router.redb`; this is why the flag defaults off and is not
    /// exposed in the UI yet.
    pub(crate) fn router_topic() -> TopicId {
        p2panda::Hash::digest(dash_router::GOSSIP_TOPIC.as_bytes()).into()
    }

    pub(crate) struct Publisher(EphemeralStreamPublisher<ByteBuf>);

    impl GossipPublisher for Publisher {
        async fn publish(&mut self, bytes: Vec<u8>) -> anyhow::Result<()> {
            self.0
                .publish(ByteBuf::from(bytes))
                .await
                .map_err(|e| anyhow!("gossip publish: {e}"))
        }
    }

    pub(crate) struct Subscription(EphemeralStreamSubscription<ByteBuf>);

    impl GossipSubscription for Subscription {
        async fn next(&mut self) -> Option<(PeerKey, Vec<u8>)> {
            let msg = self.0.next().await?;
            Some((PeerKey::from(msg.author()), msg.body().to_vec()))
        }
    }

    pub(crate) async fn open_transport(
        actor_tx: &mpsc::Sender<Command>,
    ) -> anyhow::Result<GossipTransport<Publisher, Subscription>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        actor_tx
            .send(Command::RouterStream {
                topic: router_topic(),
                reply_tx,
            })
            .await
            .map_err(|_| anyhow!("actor channel closed"))?;
        let (publisher, subscription) = reply_rx
            .await?
            .map_err(|e| anyhow!("ephemeral stream: {e}"))?;
        Ok(GossipTransport::new(
            Publisher(publisher),
            Subscription(subscription),
        ))
    }

    /// Policy for v1: the swarm test's values, which converge fifty nodes on
    /// a LAN in under a minute. Revisit with an estimator for `n`.
    const WANT_TTL: Duration = Duration::from_secs(2);
    const HAVE_TTL: Duration = Duration::from_secs(2);
    /// A relay cache sized for a phone: a payload-bearing op costs two
    /// units, so at worst ~16k ops × ~4 KB ≈ 64 MB of unvalidated peer
    /// bytes. `lan_router.redb` never shrinks and persists after the flag
    /// is turned off.
    const RELAY_CAP: Units = 1 << 15;
    const EVICT_AT: f64 = 0.75;
    const MAINTAIN_EVERY: Duration = Duration::from_secs(30);
    const NETWORK_SIZE_ESTIMATE: usize = 16;
    const IMPORT_CHANNEL: usize = 256;
    const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

    pub struct LanRouter {
        handle: RouterHandle<RouterLog>,
        ext: OpStoreExt,
        actor_tx: mpsc::Sender<Command>,
        node_task: Mutex<Option<JoinHandle<anyhow::Result<()>>>>,
        events_task: Mutex<Option<JoinHandle<()>>>,
    }

    impl LanRouter {
        pub async fn spawn(params: LanRouterParams) -> anyhow::Result<Option<Arc<Self>>> {
            if !params.enabled {
                tracing::info!("lan router off: NodeConfig::enable_lan_router is false");
                return Ok(None);
            }
            let relay =
                DiskRelayStore::<RouterLog>::open(&params.data_path.join("lan_router.redb"))
                    .context("opening lan_router.redb")?;
            let ext = OpStoreExt::new(params.op_store);
            let transport = open_transport(&params.actor_tx).await?;
            let config = CoreConfig {
                router: RouterConfig {
                    want_ttl: WANT_TTL.into(),
                    have_ttl: HAVE_TTL.into(),
                },
                relay_cap: RELAY_CAP,
                evict_at: EVICT_AT,
                debounce: PushDebouncePolicy {
                    window_ms: params.push_debounce.as_millis() as u64,
                    max_latency_ms: params.push_max_latency.as_millis() as u64,
                },
                max_wire_bytes: dash_router::pack::DEFAULT_MAX_WIRE_BYTES,
            };
            let intervals = PolicyIntervals {
                want: IntervalPolicy::Fixed {
                    min_ms: 1_000.0,
                    max_ms: 10_000.0,
                },
                have: IntervalPolicy::Fixed {
                    min_ms: 1_000.0,
                    max_ms: 10_000.0,
                },
                n: NETWORK_SIZE_ESTIMATE,
                rng: rand::rngs::StdRng::from_os_rng(),
            };
            let (handle, mut events, node_task) = dash_router::spawn(
                params.device_id,
                config,
                MAINTAIN_EVERY,
                BTreeSet::new(),
                ext.clone(),
                relay,
                transport,
                intervals,
            );
            let events_task = tokio::spawn(async move {
                while let Some(event) = events.recv().await {
                    match event {
                        RouterEvent::Delivered(log, seq) => {
                            tracing::debug!(%log, seq, "lan router delivered an op")
                        }
                        // Ingest failures are mostly peers' bad ops, so a hostile
                        // peer could flood the log with them at warn.
                        RouterEvent::StorageError(e) if e.context.ends_with("ext.ingest") => {
                            tracing::debug!(context = e.context, message = %e.message, "lan router ingest rejected")
                        }
                        RouterEvent::StorageError(e) => {
                            tracing::warn!(context = e.context, message = %e.message, "lan router storage error")
                        }
                    }
                }
                tracing::debug!("lan router events closed");
            });
            tracing::info!(
                device_id = %params.device_id,
                router_topic = ?router_topic().aliased(),
                "lan router running"
            );
            Ok(Some(Arc::new(Self {
                handle,
                ext,
                actor_tx: params.actor_tx,
                node_task: Mutex::new(Some(node_task)),
                events_task: Mutex::new(Some(events_task)),
            })))
        }

        /// One router channel subscription per topic: every author's log on
        /// it, known or not yet. Idempotent. A failure unregisters the
        /// topic, so a later call retries instead of reporting success.
        /// Concurrent calls for the same topic may both return Ok while the
        /// first is still in flight; callers must not rely on the second
        /// call observing the first's failure.
        pub async fn subscribe_topic(&self, topic: TopicId) -> anyhow::Result<()> {
            let (tx, rx) = mpsc::channel(IMPORT_CHANNEL);
            if !self.ext.register_topic(topic, tx) {
                return Ok(());
            }
            let result = self.import_and_subscribe(topic, rx).await;
            match &result {
                Ok(()) => tracing::debug!(topic = ?topic.aliased(), "lan router following topic"),
                Err(_) => self.ext.unregister_topic(topic),
            }
            result
        }

        async fn import_and_subscribe(
            &self,
            topic: TopicId,
            rx: mpsc::Receiver<Operation>,
        ) -> anyhow::Result<()> {
            let stream = Box::pin(ReceiverStream::new(rx));
            let (reply_tx, reply_rx) = oneshot::channel();
            self.actor_tx
                .send(Command::Import {
                    topic,
                    stream,
                    reply_tx,
                })
                .await
                .map_err(|_| anyhow!("actor channel closed"))?;
            reply_rx.await?.map_err(|e| anyhow!("import: {e}"))?;
            self.handle.subscribe(LogId::from_topic(topic)).await
        }

        /// How many ops the router ingested that were not already in the
        /// op store, handed to p2panda's import. Not the count of
        /// `RouterEvent::Delivered`, which also fires for ops native sync
        /// stored but has not acked. An op native sync stores at the same
        /// moment can still be counted.
        pub fn delivered_count(&self) -> u64 {
            self.ext.forwarded()
        }

        /// Testing: ops the relay store holds for others on `topic` (our
        /// own ops live in the ext store, never here).
        #[cfg(feature = "testing")]
        pub async fn relay_holds_topic(&self, topic: TopicId) -> anyhow::Result<bool> {
            let channel = LogId::from_topic(topic);
            Ok(self
                .handle
                .relay_held()
                .await?
                .iter()
                .any(|(log, ranges)| log.log_id() == channel && !ranges.is_empty()))
        }

        pub async fn unsubscribe_topic(&self, topic: TopicId) -> anyhow::Result<()> {
            self.ext.unregister_topic(topic);
            self.handle.unsubscribe(LogId::from_topic(topic)).await
        }

        /// An op on `topic` by `author` was acked: the router's held view
        /// of that log may have grown. Lossy by design.
        pub fn hint_changed(&self, author: VerifyingKey, topic: TopicId) {
            self.ext
                .hint(RouterLog::new(LogId::from_topic(topic), author));
        }

        /// A locally authored op was acked: hand it to the router, which
        /// pushes one Have for every op authored in the debounce window to
        /// its neighbours (DESIGN.md: "authors emit a Have for newly
        /// authored ops"). Relays park it for peers who are away. The ext
        /// ingest inside `append` is a no-op for an op p2panda already has.
        /// An op whose header and body cannot fit `max_wire_bytes` alone is
        /// never pushed (it is counted in `StatsSnapshot::oversize_drops`)
        /// and is served only on request, header-only.
        pub async fn authored(
            &self,
            operation: &ProcessedOperation<Payload>,
        ) -> anyhow::Result<()> {
            let header = operation.processed().header();
            let log = RouterLog::new(header.extensions.log_id(), operation.author());
            let op = Op {
                header: header.encode(),
                payload: operation.processed().body().map(|b| b.to_bytes()),
            };
            self.handle.append(log, header.seq_num, op).await
        }

        pub async fn shutdown(&self) {
            let _ = self.handle.clone().shutdown().await;
            if let Some(mut t) = self.node_task.lock().await.take() {
                match tokio::time::timeout(SHUTDOWN_WAIT, &mut t).await {
                    Ok(Ok(Ok(()))) => {}
                    Ok(Ok(Err(e))) => {
                        tracing::warn!(error = %e, "lan router task ended with error")
                    }
                    Ok(Err(join)) => tracing::warn!(error = %join, "lan router task failed"),
                    Err(_) => {
                        t.abort();
                        tracing::warn!("lan router task did not stop in time; aborted");
                    }
                }
            }
            // The node task's end closes the events channel, so this task
            // has normally finished already; abort covers the timeout case.
            if let Some(t) = self.events_task.lock().await.take() {
                t.abort();
            }
        }
    }

    #[cfg(test)]
    mod lan_router_tests {
        use super::*;

        /// A real p2panda node + actor, offline is fine for these tests
        /// (ephemeral streams need networking, so use the online builder
        /// with no discovery and a random network id).
        async fn params(enabled: bool) -> (LanRouterParams, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let node = p2panda::Node::builder()
                .network_id(p2panda::Topic::random().into())
                .spawn()
                .await
                .unwrap();
            let store = OpStore::from_sqlite(node.store());
            let (actor, _events) = crate::node::actor::Actor::new(node, None);
            let actor_tx = actor.spawn().await.unwrap();
            let device_id = p2panda::SigningKey::generate().verifying_key();
            (
                LanRouterParams {
                    enabled,
                    data_path: dir.path().to_path_buf(),
                    device_id,
                    op_store: store,
                    push_debounce: Duration::from_millis(50),
                    push_max_latency: Duration::from_millis(200),
                    actor_tx,
                },
                dir,
            )
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn disabled_spawns_nothing() {
            let (p, dir) = params(false).await;
            assert!(LanRouter::spawn(p).await.unwrap().is_none());
            assert!(!dir.path().join("lan_router.redb").exists());
        }

        /// Subscribing twice is a no-op; unsubscribing drops the topic.
        #[tokio::test(flavor = "multi_thread")]
        async fn subscribe_topic_is_idempotent() {
            let (p, dir) = params(true).await;
            let r = LanRouter::spawn(p).await.unwrap().expect("enabled");
            assert!(dir.path().join("lan_router.redb").exists());
            let topic = TopicId::random();
            r.subscribe_topic(topic).await.unwrap();
            r.subscribe_topic(topic).await.unwrap();
            assert!(r.ext.has_topic(&LogId::from_topic(topic)));
            r.unsubscribe_topic(topic).await.unwrap();
            assert!(!r.ext.has_topic(&LogId::from_topic(topic)));
            r.shutdown().await;
        }

        /// A failed subscribe leaves no registration behind, so a retry
        /// really retries rather than returning early as "already done".
        #[tokio::test(flavor = "multi_thread")]
        async fn failed_subscribe_rolls_back() {
            let (p, _dir) = params(true).await;
            let actor_tx = p.actor_tx.clone();
            let r = LanRouter::spawn(p).await.unwrap().expect("enabled");
            let (reply_tx, reply_rx) = oneshot::channel();
            actor_tx.send(Command::Shutdown { reply_tx }).await.unwrap();
            reply_rx.await.unwrap();
            let topic = TopicId::random();
            assert!(r.subscribe_topic(topic).await.is_err());
            assert!(!r.ext.has_topic(&LogId::from_topic(topic)));
            assert!(
                r.subscribe_topic(topic).await.is_err(),
                "retried, not skipped"
            );
            r.shutdown().await;
        }

        /// Shutdown stops the router task promptly.
        #[tokio::test(flavor = "multi_thread")]
        async fn shutdown_is_clean() {
            let (p, _dir) = params(true).await;
            let r = LanRouter::spawn(p).await.unwrap().expect("enabled");
            tokio::time::timeout(std::time::Duration::from_secs(5), r.shutdown())
                .await
                .expect("shutdown returns");
        }
    }

    #[cfg(test)]
    mod router_log_tests {
        use super::*;
        use dash_router::core::Log;
        use dash_router::disk::LogKey;

        #[test]
        fn channel_is_the_log_id_and_order_is_channel_first() {
            // Key bytes do not sort like their seeds: seed [2; 32] gives the smaller key.
            let key_a = p2panda::SigningKey::from_bytes(&[2; 32]).verifying_key();
            let key_b = p2panda::SigningKey::from_bytes(&[1; 32]).verifying_key();
            let t1 = TopicId::from([1u8; 32]);
            let t2 = TopicId::from([2u8; 32]);
            let l = RouterLog::new(LogId::from_topic(t1), key_b);
            assert_eq!(l.channel(), LogId::from_topic(t1));
            assert_eq!(l.log_id(), LogId::from_topic(t1));
            assert_eq!(l.verifying_key().unwrap(), key_b);
            // Same topic, different authors, sorts inside the topic; a later
            // topic sorts after regardless of author bytes.
            let same_topic_other_author = RouterLog::new(LogId::from_topic(t1), key_a);
            let other_topic = RouterLog::new(LogId::from_topic(t2), key_a);
            assert!(same_topic_other_author < l);
            assert!(l < other_topic || other_topic < l);
            assert_eq!(l.channel(), same_topic_other_author.channel());
            assert_ne!(l.channel(), other_topic.channel());
            // `Log::new` is the lossless inverse the nested LogRanges relies on.
            assert_eq!(<RouterLog as Log>::new(l.channel(), l.author()), l);
        }

        #[test]
        fn log_key_round_trips_64_bytes() {
            let key = p2panda::SigningKey::from_bytes(&[7; 32]).verifying_key();
            let l = RouterLog::new(LogId::from_topic(TopicId::from([9u8; 32])), key);
            let mut bytes = Vec::new();
            l.write_key(&mut bytes);
            assert_eq!(bytes.len(), <RouterLog as LogKey>::WIDTH);
            assert_eq!(RouterLog::read_key(&bytes), Some(l));
            assert_eq!(RouterLog::read_key(&bytes[..10]), None);
        }

        #[test]
        fn postcard_round_trip() {
            let key = p2panda::SigningKey::from_bytes(&[3; 32]).verifying_key();
            let l = RouterLog::new(LogId::from_topic(TopicId::from([4u8; 32])), key);
            let bytes = postcard::to_stdvec(&l).unwrap();
            assert_eq!(bytes.len(), 64, "two fixed arrays, no length prefixes");
            let back: RouterLog = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(back, l);
        }
    }

    #[cfg(test)]
    mod ext_tests {
        use std::collections::BTreeSet;

        use dash_router::core::{LogRanges, Ranges};
        use dash_router::{AsyncStorage, WatchableStorage};
        use p2panda::operation::Operation;

        use super::*;
        use crate::stores::test_support::{ack_up_to, insert, signed_op};
        use crate::{DeviceId, SeqNum};

        struct Fixture {
            ext: OpStoreExt,
            topic: TopicId,
            log_id: LogId,
            key: p2panda::SigningKey,
            import_rx: mpsc::Receiver<Operation>,
        }

        /// One topic registered, one author with seqs 0..=3 stored, acked up to `acked`.
        async fn fixture(acked: Option<SeqNum>) -> Fixture {
            fixture_n(4, acked).await
        }

        /// As `fixture`, with seqs `0..n` stored.
        async fn fixture_n(n: SeqNum, acked: Option<SeqNum>) -> Fixture {
            let store = OpStore::temporary_sqlite().await.unwrap();
            let ext = OpStoreExt::new(store.clone());
            let topic = TopicId::random();
            let log_id = LogId::from_topic(topic);
            let (tx, import_rx) = mpsc::channel(8);
            assert!(ext.register_topic(topic, tx));
            let key = p2panda::SigningKey::generate();
            let mut backlink = None;
            for seq in 0..n {
                let op = signed_op(&key, log_id, seq, backlink, &[seq as u8]);
                insert(&store, &op, &log_id).await;
                backlink = Some(op.hash);
            }
            if let Some(h) = acked {
                ack_up_to(
                    &store,
                    &topic,
                    &DeviceId::from(key.verifying_key()),
                    log_id,
                    h,
                )
                .await;
            }
            Fixture {
                ext,
                topic,
                log_id,
                key,
                import_rx,
            }
        }

        /// Only acked seqs are held: stored ops above the ack cursor are not.
        #[tokio::test]
        async fn held_stops_at_acked_height() {
            let f = fixture(Some(1)).await;
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            let all = f.ext.held_all().await.unwrap();
            assert_eq!(
                all.get(&log),
                Some(&Ranges::range(0, 2)),
                "seqs 0 and 1 only"
            );
            let of = f.ext.held_of(&BTreeSet::from([log])).await.unwrap();
            assert_eq!(of.get(&log), Some(&Ranges::range(0, 2)));
        }

        #[tokio::test]
        async fn unacked_log_is_not_held() {
            let f = fixture(None).await;
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            assert!(f.ext.held_all().await.unwrap().is_empty());
            let of = f.ext.held_of(&BTreeSet::from([log])).await.unwrap();
            assert_eq!(
                of.get(&log),
                Some(&Ranges::empty()),
                "requested-but-empty mirrors the request"
            );
        }

        #[tokio::test]
        async fn fetch_serves_acked_ops_with_encoded_headers() {
            let f = fixture(Some(2)).await;
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            let want = LogRanges::from_pairs([(log, Ranges::full())]);
            let ops = f.ext.fetch(&want).await.unwrap();
            assert_eq!(
                ops.iter().map(|(_, q, _)| *q).collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
            let header = p2panda::operation::Header::decode(&ops[1].2.header).unwrap();
            assert_eq!(header.seq_num, 1);
            assert_eq!(ops[1].2.payload.as_deref(), Some(&[1u8][..]));
        }

        #[tokio::test]
        async fn ingest_forwards_a_valid_op_to_the_topic_channel() {
            let mut f = fixture(Some(3)).await;
            let other = p2panda::SigningKey::generate();
            let op = signed_op(&other, f.log_id, 0, None, b"hello");
            let log = RouterLog::new(f.log_id, other.verifying_key());
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: op.body.as_ref().map(|b| b.to_bytes()),
            };
            f.ext.ingest(log, 0, wire).await.unwrap();
            let got = f.import_rx.recv().await.unwrap();
            assert_eq!(got.hash, op.hash);
            assert_eq!(got.body, op.body);
            assert_eq!(f.ext.forwarded(), 1);
        }

        /// An op p2panda has stored but not yet acked is novel to the
        /// router; ingesting it again is neither a forward nor a rejection.
        #[tokio::test]
        async fn ingest_skips_ops_already_stored() {
            let mut f = fixture(None).await;
            let author = DeviceId::from(f.key.verifying_key());
            let stored = f.ext.store.get_log(&author, &f.log_id, None).await.unwrap();
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            let wire = dash_router::core::Op {
                header: stored[1].header.encode(),
                payload: stored[1].body.as_ref().map(|b| b.to_bytes()),
            };
            f.ext.ingest(log, 1, wire).await.unwrap();
            assert!(f.import_rx.try_recv().is_err(), "nothing forwarded");
            assert_eq!(f.ext.rejected(), 0);
            assert_eq!(f.ext.forwarded(), 0);
        }

        /// An op whose header disagrees with the claimed log or seq is
        /// rejected and not forwarded.
        #[tokio::test]
        async fn ingest_rejects_mismatched_header() {
            let mut f = fixture(Some(3)).await;
            let other = p2panda::SigningKey::generate();
            let op = signed_op(&other, f.log_id, 0, None, b"hello");
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: None,
            };
            // Claimed under the fixture's author, but signed by `other`.
            let wrong_author = RouterLog::new(f.log_id, f.key.verifying_key());
            assert!(f.ext.ingest(wrong_author, 0, wire.clone()).await.is_err());
            // Right author, wrong seq.
            let right = RouterLog::new(f.log_id, other.verifying_key());
            assert!(f.ext.ingest(right, 5, wire.clone()).await.is_err());
            // Right author and seq, but signed under another log id.
            let elsewhere = LogId::from_topic(TopicId::random());
            let op = signed_op(&other, elsewhere, 0, None, b"hello");
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: None,
            };
            assert!(f.ext.ingest(right, 0, wire).await.is_err());
            assert_eq!(f.ext.rejected(), 3);
            assert!(f.import_rx.try_recv().is_err(), "nothing forwarded");
        }

        /// A forged signature or a swapped body is rejected before the op
        /// reaches the import channel (and so p2panda's live-sync forward).
        #[tokio::test]
        async fn ingest_rejects_bad_signature_and_body() {
            let mut f = fixture(Some(3)).await;
            let other = p2panda::SigningKey::generate();
            let log = RouterLog::new(f.log_id, other.verifying_key());
            let op = signed_op(&other, f.log_id, 0, None, b"hello");

            // (a) Tampered signature bytes.
            let mut forged = op.header.clone();
            let mut sig = forged.signature.to_bytes();
            sig[0] ^= 0x01;
            forged.signature = p2panda_core::Signature::from_bytes(&sig);
            let wire = dash_router::core::Op {
                header: forged.encode(),
                payload: op.body.as_ref().map(|b| b.to_bytes()),
            };
            assert!(f.ext.ingest(log, 0, wire).await.is_err());
            assert_eq!(f.ext.rejected(), 1);

            // (b) Valid header, body replaced by different bytes.
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: Some(b"jello".to_vec()),
            };
            assert!(f.ext.ingest(log, 0, wire).await.is_err());
            assert_eq!(f.ext.rejected(), 2);

            assert!(f.import_rx.try_recv().is_err(), "nothing forwarded");
        }

        /// An op on a log under no registered topic is rejected.
        #[tokio::test]
        async fn ingest_without_topic_mapping_is_an_error() {
            let mut f = fixture(Some(3)).await;
            let stray_log_id = LogId::from_topic(TopicId::random());
            let other = p2panda::SigningKey::generate();
            let op = signed_op(&other, stray_log_id, 0, None, b"x");
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: None,
            };
            let log = RouterLog::new(stray_log_id, other.verifying_key());
            assert!(f.ext.ingest(log, 0, wire).await.is_err());
            assert_eq!(f.ext.rejected(), 1);
        }

        /// `get_log`'s cursor is exclusive: a non-zero start must still
        /// include its first seq, and gaps in the request are respected.
        #[tokio::test]
        async fn fetch_honours_nonzero_start_and_gaps() {
            let f = fixture_n(6, Some(5)).await;
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            let seqs =
                |ops: Vec<(RouterLog, Seq, Op)>| ops.iter().map(|(_, q, _)| *q).collect::<Vec<_>>();
            let want = LogRanges::from_pairs([(log, Ranges::range(2, 4))]);
            assert_eq!(seqs(f.ext.fetch(&want).await.unwrap()), vec![2, 3]);
            let gapped = LogRanges::from_pairs([(log, Ranges::from_seqs([2, 3, 5]))]);
            assert_eq!(seqs(f.ext.fetch(&gapped).await.unwrap()), vec![2, 3, 5]);
            let empty = LogRanges::from_pairs([(log, Ranges::empty())]);
            assert!(f.ext.fetch(&empty).await.unwrap().is_empty());
        }

        /// A peer-named log whose author bytes are not a valid key is an
        /// unknown log: it must not fail the batch it arrives in.
        #[tokio::test]
        async fn bogus_author_is_unknown_not_an_error() {
            let f = fixture(Some(1)).await;
            // Not every 32-byte string is a curve point (`[0xFF; 32]` is, as
            // a non-canonical encoding); take the first `[b; 32]` that isn't.
            let author = (0u8..=255)
                .map(|b| [b; 32])
                .find(|k| VerifyingKey::from_bytes(k).is_err())
                .expect("some byte pattern is not a valid key");
            let bogus = RouterLog {
                log_id: *f.log_id.as_bytes(),
                author,
            };
            assert!(
                bogus.verifying_key().is_err(),
                "fixture needs an invalid key"
            );
            let valid = RouterLog::new(f.log_id, f.key.verifying_key());

            let held = f
                .ext
                .held_of(&BTreeSet::from([bogus, valid]))
                .await
                .unwrap();
            assert_eq!(held.get(&bogus), Some(&Ranges::empty()));
            assert_eq!(held.get(&valid), Some(&Ranges::range(0, 2)));

            let want = LogRanges::from_pairs([(bogus, Ranges::full()), (valid, Ranges::full())]);
            let ops = f.ext.fetch(&want).await.unwrap();
            assert_eq!(
                ops.iter().map(|(l, q, _)| (*l, *q)).collect::<Vec<_>>(),
                vec![(valid, 0), (valid, 1)]
            );
        }

        #[tokio::test]
        async fn hint_reaches_changed_subscribers() {
            let f = fixture(Some(3)).await;
            let mut rx = f.ext.changed();
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            f.ext.hint(log);
            assert_eq!(rx.recv().await.unwrap(), BTreeSet::from([log]));
        }

        #[tokio::test]
        async fn unregistered_topic_disappears_from_held_all() {
            let f = fixture(Some(3)).await;
            assert!(!f.ext.held_all().await.unwrap().is_empty());
            f.ext.unregister_topic(f.topic);
            assert!(f.ext.held_all().await.unwrap().is_empty());
            assert!(!f.ext.has_topic(&f.log_id));
        }
    }
}
