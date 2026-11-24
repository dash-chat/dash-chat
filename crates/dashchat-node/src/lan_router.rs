//! The Dash Router LAN gossip shell, embedded (spec
//! dash-router/docs/superpowers/specs/2026-09-22-dash-chat-lan-router-design.md §4).
//!
//! Everything is a no-op unless `LanRouterConfig::enabled` is true, which
//! needs the `lan-router` cargo feature. The node calls this module
//! at five points: `LanRouter::spawn` in `Node::init`, `subscribe_topic`
//! once a topic's stream or space is open, `topics_changed` when a space's
//! membership moves, and, after an operation is acked, `authored` for our
//! own ops (the router pushes them) or `hint_changed` for others'.
//!
//! The router knows nothing of topics: each log of a followed topic is its
//! own router channel, so following a topic is subscribing every log it has.
//! With the feature off, this file is the stub below and every call site's
//! `Option` is `None`.

use std::path::PathBuf;
use std::sync::Arc;

use p2panda::VerifyingKey;
use p2panda::streams::ProcessedOperation;

use crate::node::streams::TopicStreams;
use crate::payload::Payload;
use crate::stores::OpStore;
use crate::topic::TopicId;

#[derive(Clone, Debug)]
pub struct LanRouterConfig {
    /// Whether to run the Dash Router LAN gossip shell alongside p2panda
    /// sync. A node configured to run it fails to start if the router
    /// cannot, which includes a build without the `lan-router` cargo
    /// feature. [`crate::NodeConfig::no_p2p`] turns it off. Once running,
    /// failing to follow a topic is logged and skipped: the p2panda path
    /// never depends on the router.
    ///
    /// The router floods to every member of the node's gossip overlay on its
    /// topic, which is not LAN-scoped by construction, and relays keep what
    /// they hear in `lan_router.redb`. Operations of a plain stream, such as
    /// an inbox, are not end-to-end encrypted, so every overlay member reads
    /// them. It is not exposed in the UI yet.
    pub enabled: bool,
    /// Quiet window after a locally authored op before the router pushes a
    /// Have for every op authored in the window (dash-router
    /// `PushDebouncePolicy::window_ms`).
    pub push_debounce: std::time::Duration,
    /// Hard cap after the oldest still-pending authored op, so a steady
    /// stream of authoring still pushes (`PushDebouncePolicy::max_latency_ms`).
    pub push_max_latency: std::time::Duration,
}

impl LanRouterConfig {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            push_debounce: std::time::Duration::from_millis(50),
            push_max_latency: std::time::Duration::from_millis(200),
        }
    }
}

/// Whether a topic is a space rather than a plain stream: all the router
/// needs of the node's `LocalStore`.
#[async_trait::async_trait]
pub trait IsSpace: Send + Sync + 'static {
    async fn is_space(&self, topic: TopicId) -> anyhow::Result<bool>;
}

#[async_trait::async_trait]
impl IsSpace for crate::stores::LocalStore {
    async fn is_space(&self, topic: TopicId) -> anyhow::Result<bool> {
        self.is_space(topic).await
    }
}

/// What the node hands the router at startup.
pub struct LanRouterParams {
    pub config: LanRouterConfig,
    pub data_path: PathBuf,
    pub device_id: VerifyingKey,
    pub op_store: OpStore,
    /// Read only by the feature-on router.
    #[cfg_attr(not(feature = "lan-router"), allow(dead_code))]
    pub is_space: Arc<dyn IsSpace>,
    /// Read only by the feature-on router.
    #[cfg_attr(not(feature = "lan-router"), allow(dead_code))]
    pub(crate) streams: Arc<TopicStreams>,
}

#[cfg(not(feature = "lan-router"))]
pub struct LanRouter;

#[cfg(not(feature = "lan-router"))]
impl LanRouter {
    pub async fn spawn(params: LanRouterParams) -> anyhow::Result<Option<Arc<Self>>> {
        anyhow::ensure!(
            !params.config.enabled,
            "the LAN router is enabled but this build has no lan-router feature"
        );
        Ok(None)
    }
    pub async fn subscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
        Ok(())
    }
    pub async fn unsubscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn hint_changed(&self, _author: VerifyingKey, _log_id: p2panda::operation::LogId) {}
    pub fn topics_changed(&self) {}
    pub fn authored(&self, _operation: &ProcessedOperation<Payload>) {}
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
    use tokio::sync::{Mutex, Notify, broadcast, mpsc};
    use tokio::task::JoinHandle;
    use tokio_stream::wrappers::ReceiverStream;

    use crate::stores::{LanRouterStore, LogSeqSummary};
    use crate::{DeviceId, SeqNum};

    /// The router's log identity: `(LogId, author)`, channel first so the
    /// relay store's ordered scans keep one topic's logs contiguous. The
    /// channel (one of a topic's logs) is what a subscription names.
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

    /// A [`RouterLog`]'s channel: the raw bytes of a `LogId`.
    #[derive(
        Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug,
    )]
    pub(crate) struct ChannelId([u8; 32]);

    impl From<LogId> for ChannelId {
        fn from(log_id: LogId) -> Self {
            Self(*log_id.as_bytes())
        }
    }

    impl std::fmt::Display for ChannelId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", hex::encode(&self.0[..4]))
        }
    }

    impl dash_router::core::Log for RouterLog {
        type Channel = ChannelId;
        type Author = AuthorKey;
        fn channel(&self) -> ChannelId {
            ChannelId(self.log_id)
        }
        fn author(&self) -> AuthorKey {
            AuthorKey(self.author)
        }
        fn new(channel: ChannelId, author: AuthorKey) -> Self {
            Self {
                log_id: channel.0,
                author: author.0,
            }
        }
        fn author_prefix(author: &AuthorKey) -> u32 {
            u32::from_be_bytes([author.0[0], author.0[1], author.0[2], author.0[3]])
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

    /// What the router follows. A topic's logs are each their own router
    /// channel, and a log can belong to several topics (a member's key
    /// bundle log is one log across every space they are in).
    #[derive(Default)]
    struct Followed {
        /// The channel each topic's imported ops go down (consumed by
        /// `TopicStreams::import`).
        imports: HashMap<TopicId, mpsc::Sender<Operation>>,
        logs: HashMap<LogId, BTreeSet<TopicId>>,
    }

    /// Which of a log's stored seqs may be held and served.
    enum Sendable {
        All,
        /// Only up to the ack cursor `topic` keeps for the log.
        Acked(TopicId),
    }

    /// The router's ext store: Dash Chat's own `OpStore`, seen as
    /// `(LogId, author)` logs. Only logs of a followed topic exist for the
    /// router. What is held and served follows the mailbox's rule
    /// (`OpStore::sendable_log_segment`): a plain stream's log only up to
    /// its ack cursor, a space's logs in full, since their ops carry no
    /// payload a tombstone could still drop.
    #[derive(Clone)]
    pub(crate) struct OpStoreExt {
        store: LanRouterStore,
        /// Read on every held/fetch/ingest; written on (un)subscribe. Never
        /// held across an await.
        followed: Arc<RwLock<Followed>>,
        hints: broadcast::Sender<BTreeSet<RouterLog>>,
        /// Notified when a followed topic may have gained logs.
        topics_changed: Arc<Notify>,
        rejected: Arc<AtomicU64>,
        forwarded: Arc<AtomicU64>,
    }

    impl OpStoreExt {
        pub(crate) fn new(store: LanRouterStore) -> Self {
            Self {
                store,
                followed: Default::default(),
                hints: broadcast::channel(64).0,
                topics_changed: Default::default(),
                rejected: Default::default(),
                forwarded: Default::default(),
            }
        }

        /// Follow a topic, importing its ops down `import_tx`. Returns
        /// false if it is already followed.
        pub(crate) fn follow_topic(
            &self,
            topic: TopicId,
            import_tx: mpsc::Sender<Operation>,
        ) -> bool {
            use std::collections::hash_map::Entry;
            match self.followed.write().unwrap().imports.entry(topic) {
                Entry::Occupied(_) => false,
                Entry::Vacant(e) => {
                    e.insert(import_tx);
                    true
                }
            }
        }

        /// Count `log_id` as one of a followed topic's logs. Returns true
        /// if no topic had it before, so the router is not subscribed yet.
        pub(crate) fn follow_log(&self, topic: TopicId, log_id: LogId) -> bool {
            let mut followed = self.followed.write().unwrap();
            if !followed.imports.contains_key(&topic) {
                return false;
            }
            let topics = followed.logs.entry(log_id).or_default();
            let new = topics.is_empty();
            topics.insert(topic);
            new
        }

        /// Stop following a topic. Returns the logs no other topic has.
        pub(crate) fn unfollow_topic(&self, topic: TopicId) -> Vec<LogId> {
            let mut followed = self.followed.write().unwrap();
            followed.imports.remove(&topic);
            let mut orphaned = Vec::new();
            followed.logs.retain(|log_id, topics| {
                if topics.remove(&topic) && topics.is_empty() {
                    orphaned.push(*log_id);
                }
                !topics.is_empty()
            });
            orphaned
        }

        #[cfg(test)]
        pub(crate) fn has_log(&self, log_id: &LogId) -> bool {
            self.followed.read().unwrap().logs.contains_key(log_id)
        }

        pub(crate) fn hint(&self, log: RouterLog) {
            let _ = self.hints.send(BTreeSet::from([log])); // no receivers is fine
        }

        /// Any followed log may hold more: have the router re-read them all.
        fn hint_all(&self) {
            let _ = self.hints.send(BTreeSet::new());
        }

        fn followed_topics(&self) -> Vec<TopicId> {
            self.followed
                .read()
                .unwrap()
                .imports
                .keys()
                .copied()
                .collect()
        }

        #[cfg(test)]
        pub(crate) fn rejected(&self) -> u64 {
            self.rejected.load(Ordering::Relaxed)
        }

        /// Ops ingested that were not in the op store yet, handed to p2panda.
        pub(crate) fn forwarded(&self) -> u64 {
            self.forwarded.load(Ordering::Relaxed)
        }

        /// `None` when no followed topic has the log.
        fn sendable_of(&self, log_id: &LogId) -> Option<Sendable> {
            let followed = self.followed.read().unwrap();
            let topics = followed.logs.get(log_id)?;
            let stream = topics
                .iter()
                .find(|topic| LogId::from_topic(**topic) == *log_id);
            Some(stream.map_or(Sendable::All, |topic| Sendable::Acked(*topic)))
        }

        fn import_txs_of(&self, log_id: &LogId) -> Vec<mpsc::Sender<Operation>> {
            let followed = self.followed.read().unwrap();
            let Some(topics) = followed.logs.get(log_id) else {
                return Vec::new();
            };
            topics
                .iter()
                .filter_map(|topic| followed.imports.get(topic).cloned())
                .collect()
        }

        /// The highest seq of `author`'s log that may be served: `Ok(None)`
        /// for no limit, `Err(())` when nothing may be.
        async fn sendable_height(
            &self,
            sendable: &Sendable,
            author: &DeviceId,
            log_id: &LogId,
        ) -> anyhow::Result<Result<Option<SeqNum>, ()>> {
            let Sendable::Acked(topic) = sendable else {
                return Ok(Ok(None));
            };
            let acked = self.store.acked_log_height(topic, author, log_id).await?;
            Ok(acked.map(Some).ok_or(()))
        }

        /// Sendable seqs present for one log, as ranges; empty when nothing
        /// is sendable, no followed topic has the log, or the author bytes
        /// are not a valid key (a peer can name such a log; it is unknown,
        /// not an error).
        async fn held_one(&self, log: &RouterLog) -> anyhow::Result<Ranges> {
            let log_id = log.log_id();
            let Some(sendable) = self.sendable_of(&log_id) else {
                return Ok(Ranges::empty());
            };
            let Ok(author) = log.verifying_key() else {
                return Ok(Ranges::empty());
            };
            let author = DeviceId::from(author);
            let Ok(height) = self.sendable_height(&sendable, &author, &log_id).await? else {
                return Ok(Ranges::empty());
            };
            let Some(summary) = self.store.get_log_seq_summary(&author, &log_id).await? else {
                return Ok(Ranges::empty());
            };
            self.held_below(&author, &log_id, summary, height).await
        }

        /// The seqs of `summary` at or below `height`, all of them when it
        /// is `None`. Acks are in log order and only a prefix is ever
        /// pruned, so the stored seqs form one run and the summary alone
        /// gives the answer; a gap (not expected) falls back to listing
        /// every seq.
        async fn held_below(
            &self,
            author: &DeviceId,
            log_id: &LogId,
            summary: LogSeqSummary,
            height: Option<SeqNum>,
        ) -> anyhow::Result<Ranges> {
            let top = height.map_or(summary.max, |height| summary.max.min(height));
            if summary.min > top {
                return Ok(Ranges::empty());
            }
            if summary.is_contiguous() {
                return Ok(Ranges::range(summary.min, top + 1));
            }
            let seqs = self.store.get_log_seqs(author, log_id).await?;
            Ok(Ranges::from_seqs(seqs.into_iter().filter(|q| *q <= top)))
        }

        /// Every author's sendable seqs in one followed log.
        async fn held_in_log(
            &self,
            log_id: LogId,
            out: &mut LogRanges<RouterLog>,
        ) -> anyhow::Result<()> {
            let Some(sendable) = self.sendable_of(&log_id) else {
                return Ok(());
            };
            let acked = match &sendable {
                Sendable::All => None,
                Sendable::Acked(topic) => Some(self.store.acked_log_heights(topic, &log_id).await?),
            };
            for (author, summary) in self.store.get_log_seq_summaries(&log_id).await? {
                let height = match &acked {
                    None => None,
                    Some(acked) => match acked.get(&author) {
                        Some(height) => Some(*height),
                        None => continue,
                    },
                };
                let r = self.held_below(&author, &log_id, summary, height).await?;
                if !r.is_empty() {
                    out.insert(RouterLog::new(log_id, *author), r);
                }
            }
            Ok(())
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
            let log_ids: Vec<LogId> = self.followed.read().unwrap().logs.keys().copied().collect();
            let mut out = LogRanges::empty();
            for log_id in log_ids {
                self.held_in_log(log_id, &mut out).await?;
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
                let log_id = log.log_id();
                let Some(sendable) = self.sendable_of(&log_id) else {
                    continue;
                };
                // A bogus author is an unknown log, not a batch failure.
                let Ok(author) = log.verifying_key() else {
                    continue;
                };
                let author = DeviceId::from(author);
                let Ok(height) = self.sendable_height(&sendable, &author, &log_id).await? else {
                    continue;
                };
                // The store's lower bound is exclusive ("after"): start one
                // below the first wanted seq, or from the beginning when
                // that is 0. An open-ended want reads up to the sendable
                // height.
                let after = r.boundaries().first().and_then(|s| s.checked_sub(1));
                let until = match (r.last(), height) {
                    (Some(last), Some(height)) => Some(last.min(height)),
                    (last, height) => last.or(height),
                };
                for op in self
                    .store
                    .get_log_until(&author, &log_id, after, until)
                    .await?
                {
                    let seq = op.header.seq_num;
                    if !r.contains(seq) {
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
                // A stream's held state is acked-only, so the router treats
                // ops p2panda stored but has not acked yet as novel and
                // ingests them again.
                if self.store.has_operation(&hash).await? {
                    tracing::trace!(%log, seq, "lan router ingest: op already stored");
                    return Ok(());
                }
                let txs = self.import_txs_of(&log.log_id());
                ensure!(!txs.is_empty(), "no topic for log {log}");
                let operation = Operation {
                    hash,
                    header,
                    body: op.payload.as_deref().map(Body::from_bytes),
                };
                // Signature and payload hash/size, as p2panda's ingest checks:
                // p2panda forwards imported ops to live-sync peers before its
                // own pipeline validates them, so a forgery must stop here.
                p2panda_core::validate_operation(&operation).context("invalid operation")?;
                // An import assigns the op to its topic, so a log several
                // topics share goes to each of them.
                for tx in txs {
                    tx.send(operation.clone())
                        .await
                        .map_err(|_| anyhow!("import channel closed"))?;
                }
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

    /// The logs a space has before any op of it is stored.
    fn space_logs(topic: TopicId) -> [LogId; 2] {
        [
            crate::topic::application_log_id(topic),
            p2panda::spaces::space_log_id(topic.into()),
        ]
    }

    pub(crate) async fn open_transport(
        streams: &TopicStreams,
    ) -> anyhow::Result<GossipTransport<Publisher, Subscription>> {
        let (publisher, subscription) = streams
            .ephemeral_stream::<ByteBuf>(router_topic())
            .await
            .map_err(|e| anyhow!("ephemeral stream: {e}"))?;
        Ok(GossipTransport::new(
            Publisher(publisher),
            Subscription(subscription),
        ))
    }

    /// Policy for v1: the swarm test's values, which converge fifty nodes on
    /// a LAN in under a minute. Revisit with an estimator for `n`.
    const TIMER_MIN: Duration = Duration::from_secs(1);
    const TIMER_MAX: Duration = Duration::from_secs(10);
    /// A Want is answered when the Have timer fires and a heard channel is
    /// asked about when the Want timer does, so both must outlive the
    /// longest timer.
    const WANT_TTL: Duration = Duration::from_secs(12);
    const HAVE_TTL: Duration = Duration::from_secs(2);
    const HEARD_TTL: Duration = Duration::from_secs(12);
    /// A relay cache sized for a phone: a payload-bearing op costs two
    /// units, so at worst ~16k ops × ~4 KB ≈ 64 MB of unvalidated peer
    /// bytes. `lan_router.redb` never shrinks and persists after the flag
    /// is turned off.
    const RELAY_CAP: Units = 1 << 15;
    const EVICT_AT: f64 = 0.75;
    const MAINTAIN_EVERY: Duration = Duration::from_secs(30);
    const NETWORK_SIZE_ESTIMATE: usize = 16;
    const IMPORT_CHANNEL: usize = 256;
    /// How often the op store is checked for operations stored since the
    /// last refresh pass.
    const STORE_POLL_EVERY: Duration = Duration::from_secs(1);
    /// A pass also runs this often unprompted.
    const REFRESH_EVERY: Duration = Duration::from_secs(30);
    const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

    pub struct LanRouter {
        handle: RouterHandle<RouterLog>,
        ext: OpStoreExt,
        streams: Arc<TopicStreams>,
        is_space: Arc<dyn IsSpace>,
        node_task: Mutex<Option<JoinHandle<anyhow::Result<()>>>>,
        events_task: Mutex<Option<JoinHandle<()>>>,
        refresh_task: Mutex<Option<JoinHandle<()>>>,
    }

    /// Keep `router`'s subscriptions and held view in step with the op
    /// store. p2panda writes a space's control operations there without an
    /// event or an ack, some of them after the call that caused them has
    /// returned, so the store itself is watched.
    async fn refresh_loop(router: std::sync::Weak<LanRouter>, changed: Arc<Notify>) {
        let mut seen_rowid = None;
        let mut last_pass = tokio::time::Instant::now();
        loop {
            let notified = tokio::time::timeout(STORE_POLL_EVERY, changed.notified())
                .await
                .is_ok();
            let Some(router) = router.upgrade() else {
                return;
            };
            let rowid = router
                .ext
                .store
                .latest_operation_rowid()
                .await
                .ok()
                .flatten();
            if !notified && rowid == seen_rowid && last_pass.elapsed() < REFRESH_EVERY {
                continue;
            }
            router.refresh().await;
            seen_rowid = rowid;
            last_pass = tokio::time::Instant::now();
        }
    }

    impl LanRouter {
        pub async fn spawn(params: LanRouterParams) -> anyhow::Result<Option<Arc<Self>>> {
            if !params.config.enabled {
                tracing::info!("lan router off: LanRouterConfig::enabled is false");
                return Ok(None);
            }
            let relay =
                DiskRelayStore::<RouterLog>::open(&params.data_path.join("lan_router.redb"))
                    .context("opening lan_router.redb")?;
            let ext = OpStoreExt::new(LanRouterStore::new(params.op_store.store.clone()));
            let transport = open_transport(&params.streams).await?;
            let config = CoreConfig {
                router: RouterConfig {
                    want_ttl: WANT_TTL.into(),
                    have_ttl: HAVE_TTL.into(),
                    heard_ttl: HEARD_TTL.into(),
                },
                relay_cap: RELAY_CAP,
                evict_at: EVICT_AT,
                debounce: PushDebouncePolicy {
                    window_ms: params.config.push_debounce.as_millis() as u64,
                    max_latency_ms: params.config.push_max_latency.as_millis() as u64,
                },
                // Derived from the ephemeral stream's gossip limit.
                max_wire_bytes: None,
            };
            let intervals = PolicyIntervals {
                want: IntervalPolicy::Fixed {
                    min_ms: TIMER_MIN.as_millis() as f64,
                    max_ms: TIMER_MAX.as_millis() as f64,
                },
                have: IntervalPolicy::Fixed {
                    min_ms: TIMER_MIN.as_millis() as f64,
                    max_ms: TIMER_MAX.as_millis() as f64,
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
            let changed = ext.topics_changed.clone();
            let router = Arc::new(Self {
                handle,
                ext,
                streams: params.streams,
                is_space: params.is_space,
                node_task: Mutex::new(Some(node_task)),
                events_task: Mutex::new(Some(events_task)),
                refresh_task: Mutex::new(None),
            });
            let refresh_task = tokio::spawn(refresh_loop(Arc::downgrade(&router), changed));
            *router.refresh_task.lock().await = Some(refresh_task);
            Ok(Some(router))
        }

        /// Follow `topic`: one router channel subscription per log it has,
        /// each covering every author's log on it, known or not yet. A
        /// topic gains logs over time; [`Self::topics_changed`] and the
        /// refresh pass pick those up. Idempotent.
        ///
        /// A failure unfollows the topic, so a later call retries instead of
        /// reporting success; unfollowing drops the import channel's only
        /// sender, which ends the stream handed to p2panda, so a retry's
        /// fresh import never runs beside a stale one (p2panda would allow
        /// it anyway: each external stream is its own session).
        /// Concurrent calls for the same topic may both return Ok while the
        /// first is still in flight; callers must not rely on the second
        /// call observing the first's failure.
        pub async fn subscribe_topic(&self, topic: TopicId) -> anyhow::Result<()> {
            let result = self.import_and_subscribe(topic).await;
            match &result {
                Ok(()) => tracing::debug!(topic = ?topic.aliased(), "lan router following topic"),
                Err(_) => self.unfollow(topic).await,
            }
            self.ext.hint_all();
            result
        }

        async fn import_and_subscribe(&self, topic: TopicId) -> anyhow::Result<()> {
            let (tx, rx) = mpsc::channel(IMPORT_CHANNEL);
            if self.ext.follow_topic(topic, tx) {
                let stream = Box::pin(ReceiverStream::new(rx));
                self.streams
                    .import(topic, crate::node::streams::ImportOrigin::LanRouter, stream)
                    .await
                    .map_err(|e| anyhow!("import: {e}"))?;
            }
            self.subscribe_logs(topic).await
        }

        /// Subscribe every log of a followed topic that is not yet.
        async fn subscribe_logs(&self, topic: TopicId) -> anyhow::Result<()> {
            for log_id in self.topic_logs(topic).await? {
                if self.ext.follow_log(topic, log_id) {
                    self.handle.subscribe(log_id.into()).await?;
                }
            }
            Ok(())
        }

        /// Every log `topic` has. Beyond the ones p2panda has associated
        /// with it, that is the ones a node that has stored nothing of them
        /// yet must still ask for: a plain stream's only log, or a space's
        /// own logs, the key bundle log of each author seen in it and the
        /// log of each group its control ops name.
        async fn topic_logs(&self, topic: TopicId) -> anyhow::Result<BTreeSet<LogId>> {
            let store = &self.ext.store;
            let by_author = store.topic_logs(&topic).await?;
            let mut logs: BTreeSet<LogId> = by_author.values().flatten().copied().collect();
            if !self.is_space.is_space(topic).await? {
                logs.insert(LogId::from_topic(topic));
                return Ok(logs);
            }
            logs.extend(space_logs(topic));
            logs.extend(
                by_author
                    .keys()
                    .map(|author| p2panda::spaces::member_log_id(**author)),
            );
            logs.extend(store.space_group_log_ids(&topic, by_author.keys()).await?);
            Ok(logs)
        }

        /// A followed topic may have gained logs without an operation being
        /// stored: look them over again now.
        pub fn topics_changed(&self) {
            self.ext.topics_changed.notify_one();
        }

        async fn refresh(&self) {
            for topic in self.ext.followed_topics() {
                if let Err(e) = self.subscribe_logs(topic).await {
                    tracing::debug!(topic = ?topic.aliased(), error = %e, "lan router could not refresh a topic's logs");
                }
            }
            self.ext.hint_all();
            tracing::trace!("lan router refreshed its topics' logs");
        }

        /// Unfollow `topic` and unsubscribe the logs only it had.
        async fn unfollow(&self, topic: TopicId) {
            for log_id in self.ext.unfollow_topic(topic) {
                if let Err(e) = self.handle.unsubscribe(log_id.into()).await {
                    tracing::debug!(error = %e, "lan router could not unsubscribe a log");
                }
            }
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
            // A relay follows none of the topic's logs, so it cannot tell a
            // space from a stream: look under every log either starts with.
            let mut logs = BTreeSet::from(space_logs(topic));
            logs.insert(LogId::from_topic(topic));
            Ok(self
                .handle
                .relay_held()
                .await?
                .iter()
                .any(|(log, ranges)| logs.contains(&log.log_id()) && !ranges.is_empty()))
        }

        /// Testing: whether the relay store holds everything `other`'s
        /// router serves.
        #[cfg(feature = "testing")]
        pub async fn relay_covers(&self, other: &LanRouter) -> anyhow::Result<bool> {
            let served = other.ext.held_all().await?;
            Ok(served
                .difference(&self.handle.relay_held().await?)
                .is_empty())
        }

        pub async fn unsubscribe_topic(&self, topic: TopicId) -> anyhow::Result<()> {
            self.unfollow(topic).await;
            Ok(())
        }

        /// An op in `author`'s `log_id` was acked: the router's held view
        /// of that log may have grown. Lossy by design.
        pub fn hint_changed(&self, author: VerifyingKey, log_id: LogId) {
            self.ext.hint(RouterLog::new(log_id, author));
        }

        /// A locally authored op was acked: hand it to the router, which
        /// pushes one Have for every op authored in the debounce window to
        /// its neighbours (DESIGN.md: "authors emit a Have for newly
        /// authored ops"). Relays park it for peers who are away. The ext
        /// ingest inside `append` is a no-op for an op p2panda already has.
        /// An op whose header and body cannot fit `max_wire_bytes` alone is
        /// never pushed (it is counted in `StatsSnapshot::oversize_drops`)
        /// and is served only on request, header-only.
        ///
        /// Returns at once: the hand-off runs on its own task so a full
        /// router channel never stalls the caller's ack loop. If the router
        /// refuses the op, the log is hinted instead (lossy fallback).
        pub fn authored(&self, operation: &ProcessedOperation<Payload>) {
            let header = operation.processed().header();
            let log = RouterLog::new(header.extensions.log_id(), operation.author());
            let seq = header.seq_num;
            let op = Op {
                header: header.encode(),
                payload: operation.processed().body().map(|b| b.to_bytes()),
            };
            let handle = self.handle.clone();
            let ext = self.ext.clone();
            tokio::spawn(async move {
                if let Err(e) = handle.append(log, seq, op).await {
                    tracing::warn!(error = %e, %log, seq, "lan router could not push an authored op");
                    ext.hint(log);
                }
            });
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
            if let Some(t) = self.refresh_task.lock().await.take() {
                t.abort();
            }
        }
    }

    #[cfg(test)]
    mod lan_router_tests {
        use super::*;

        /// These tests follow plain streams only.
        struct NoSpaces;

        #[async_trait::async_trait]
        impl IsSpace for NoSpaces {
            async fn is_space(&self, _topic: TopicId) -> anyhow::Result<bool> {
                Ok(false)
            }
        }

        /// A real p2panda node, offline is fine for these tests (ephemeral
        /// streams need networking, so use the online builder with no
        /// discovery and a random network id). The drain is returned so the
        /// streams stay open for as long as the test holds it.
        async fn params(
            enabled: bool,
        ) -> (
            LanRouterParams,
            tempfile::TempDir,
            crate::node::streams::TopicDrain,
        ) {
            let dir = tempfile::tempdir().unwrap();
            let node = p2panda::Node::builder()
                .network_id(p2panda::Topic::random().into())
                .spawn()
                .await
                .unwrap();
            let store = node.store();
            let (streams, drain) = TopicStreams::new(node, None);
            let streams = Arc::new(streams);
            let device_id = p2panda::SigningKey::generate().verifying_key();
            (
                LanRouterParams {
                    config: LanRouterConfig::new(enabled),
                    data_path: dir.path().to_path_buf(),
                    device_id,
                    op_store: OpStore::from_sqlite(store, streams.clone()),
                    streams,
                    is_space: Arc::new(NoSpaces),
                },
                dir,
                drain,
            )
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn disabled_spawns_nothing() {
            let (p, dir, _drain) = params(false).await;
            assert!(LanRouter::spawn(p).await.unwrap().is_none());
            assert!(!dir.path().join("lan_router.redb").exists());
        }

        /// Subscribing twice is a no-op; unsubscribing drops the topic.
        #[tokio::test(flavor = "multi_thread")]
        async fn subscribe_topic_is_idempotent() {
            let (p, dir, _drain) = params(true).await;
            let streams = p.streams.clone();
            let r = LanRouter::spawn(p).await.unwrap().expect("enabled");
            assert!(dir.path().join("lan_router.redb").exists());
            let topic = TopicId::random();
            streams.subscribe(topic).await.unwrap();
            r.subscribe_topic(topic).await.unwrap();
            r.subscribe_topic(topic).await.unwrap();
            assert!(r.ext.has_log(&LogId::from_topic(topic)));
            r.unsubscribe_topic(topic).await.unwrap();
            assert!(!r.ext.has_log(&LogId::from_topic(topic)));
            r.shutdown().await;
        }

        /// A failed subscribe leaves no registration behind, so a retry
        /// really retries rather than returning early as "already done".
        #[tokio::test(flavor = "multi_thread")]
        async fn failed_subscribe_rolls_back() {
            let (p, _dir, _drain) = params(true).await;
            let streams = p.streams.clone();
            let r = LanRouter::spawn(p).await.unwrap().expect("enabled");
            streams.shutdown().await;
            let topic = TopicId::random();
            assert!(r.subscribe_topic(topic).await.is_err());
            assert!(!r.ext.has_log(&LogId::from_topic(topic)));
            assert!(
                r.subscribe_topic(topic).await.is_err(),
                "retried, not skipped"
            );
            r.shutdown().await;
        }

        /// Shutdown stops the router task promptly.
        #[tokio::test(flavor = "multi_thread")]
        async fn shutdown_is_clean() {
            let (p, _dir, _drain) = params(true).await;
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
            assert_eq!(l.channel(), LogId::from_topic(t1).into());
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
            node: crate::testing::TestNode,
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
            let node =
                crate::testing::TestNode::new(crate::NodeConfig::testing().no_p2p(), "ext").await;
            let store = node.op_store.clone();
            let ext = OpStoreExt::new(LanRouterStore::new(store.store.clone()));
            let topic = TopicId::random();
            let log_id = LogId::from_topic(topic);
            let (tx, import_rx) = mpsc::channel(8);
            assert!(ext.follow_topic(topic, tx));
            assert!(ext.follow_log(topic, log_id));
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
                node,
                ext,
                topic,
                log_id,
                key,
                import_rx,
            }
        }

        /// Only acked seqs are held: stored ops above the ack cursor are not.
        #[tokio::test(flavor = "multi_thread")]
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

        /// A pruned prefix leaves one run above it; a gap inside the acked
        /// part (never expected) still yields exactly the stored seqs.
        #[tokio::test(flavor = "multi_thread")]
        async fn held_follows_a_pruned_prefix_and_a_gap() {
            let f = fixture_n(0, None).await;
            let author = DeviceId::from(f.key.verifying_key());
            let mut backlink = None;
            for seq in 0..6 {
                let op = signed_op(&f.key, f.log_id, seq, backlink, &[seq as u8]);
                backlink = Some(op.hash);
                if seq >= 2 && seq != 4 {
                    insert(&f.node.op_store, &op, &f.log_id).await;
                }
            }
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            ack_up_to(&f.node.op_store, &f.topic, &author, f.log_id, 3).await;
            assert_eq!(
                f.ext.held_all().await.unwrap().get(&log),
                Some(&Ranges::range(2, 4)),
                "prefix 0..2 pruned, 2 and 3 acked"
            );
            ack_up_to(&f.node.op_store, &f.topic, &author, f.log_id, 5).await;
            let of = f.ext.held_of(&BTreeSet::from([log])).await.unwrap();
            assert_eq!(
                of.get(&log),
                Some(&Ranges::from_seqs([2, 3, 5])),
                "gap at 4 is not claimed"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
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

        #[tokio::test(flavor = "multi_thread")]
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

        #[tokio::test(flavor = "multi_thread")]
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
        #[tokio::test(flavor = "multi_thread")]
        async fn ingest_skips_ops_already_stored() {
            let mut f = fixture(None).await;
            let author = DeviceId::from(f.key.verifying_key());
            let stored = f
                .ext
                .store
                .get_log_until(&author, &f.log_id, None, None)
                .await
                .unwrap();
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
        #[tokio::test(flavor = "multi_thread")]
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
        #[tokio::test(flavor = "multi_thread")]
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
        #[tokio::test(flavor = "multi_thread")]
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
        #[tokio::test(flavor = "multi_thread")]
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
        #[tokio::test(flavor = "multi_thread")]
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

        #[tokio::test(flavor = "multi_thread")]
        async fn hint_reaches_changed_subscribers() {
            let f = fixture(Some(3)).await;
            let mut rx = f.ext.changed();
            let log = RouterLog::new(f.log_id, f.key.verifying_key());
            f.ext.hint(log);
            assert_eq!(rx.recv().await.unwrap(), BTreeSet::from([log]));
        }

        /// Only a stream's own log waits for the ack cursor: any other log
        /// of a topic (a space's) is held as soon as it is stored.
        #[tokio::test(flavor = "multi_thread")]
        async fn a_log_other_than_the_streams_own_is_held_unacked() {
            let f = fixture(None).await;
            let space_log = LogId::from_topic(TopicId::random());
            assert!(f.ext.follow_log(f.topic, space_log));
            let op0 = signed_op(&f.key, space_log, 0, None, b"a");
            let op1 = signed_op(&f.key, space_log, 1, Some(op0.hash), b"b");
            insert(&f.node.op_store, &op0, &space_log).await;
            insert(&f.node.op_store, &op1, &space_log).await;

            let log = RouterLog::new(space_log, f.key.verifying_key());
            let held = f.ext.held_all().await.unwrap();
            assert_eq!(held.get(&log), Some(&Ranges::range(0, 2)));
            assert_eq!(
                held.get(&RouterLog::new(f.log_id, f.key.verifying_key())),
                None,
                "the stream's own log is still unacked"
            );
            let mut want = LogRanges::empty();
            want.insert(log, Ranges::range(0, 2));
            let served = f.ext.fetch(&want).await.unwrap();
            assert_eq!(
                served.iter().map(|(_, seq, _)| *seq).collect::<Vec<_>>(),
                vec![0, 1]
            );
        }

        /// A log two topics share is subscribed once, imported into both,
        /// and kept until the last of them is unfollowed.
        #[tokio::test(flavor = "multi_thread")]
        async fn a_shared_log_is_imported_into_each_topic() {
            let mut f = fixture(Some(3)).await;
            let second = TopicId::random();
            let (tx, mut second_rx) = mpsc::channel(8);
            assert!(f.ext.follow_topic(second, tx));
            assert!(
                !f.ext.follow_log(second, f.log_id),
                "the router is already subscribed"
            );

            let other = p2panda::SigningKey::generate();
            let op = signed_op(&other, f.log_id, 0, None, b"hello");
            let wire = dash_router::core::Op {
                header: op.header.encode(),
                payload: op.body.as_ref().map(|b| b.to_bytes()),
            };
            let log = RouterLog::new(f.log_id, other.verifying_key());
            f.ext.ingest(log, 0, wire).await.unwrap();
            assert_eq!(f.import_rx.recv().await.unwrap().hash, op.hash);
            assert_eq!(second_rx.recv().await.unwrap().hash, op.hash);

            assert_eq!(f.ext.unfollow_topic(f.topic), vec![]);
            assert!(f.ext.has_log(&f.log_id));
            assert_eq!(f.ext.unfollow_topic(second), vec![f.log_id]);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn unregistered_topic_disappears_from_held_all() {
            let f = fixture(Some(3)).await;
            assert!(!f.ext.held_all().await.unwrap().is_empty());
            assert_eq!(f.ext.unfollow_topic(f.topic), vec![f.log_id]);
            assert!(f.ext.held_all().await.unwrap().is_empty());
            assert!(!f.ext.has_log(&f.log_id));
        }
    }
}
