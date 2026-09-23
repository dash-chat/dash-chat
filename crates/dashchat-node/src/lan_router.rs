//! The Dash Router LAN gossip shell, embedded (spec
//! dash-router/docs/superpowers/specs/2026-09-22-dash-chat-lan-router-design.md §4).
//!
//! Everything is a no-op unless BOTH the `lan-router` cargo feature is on
//! and `NodeConfig::enable_lan_router` is true. The node calls this module
//! at three points: `LanRouter::spawn` in `Node::init`, `subscribe_topic`
//! from `initialize_topic`, and `hint_changed` after an operation is
//! acked. With the feature off, this file is the stub below and every call
//! site's `Option` is `None`.

use std::path::PathBuf;
use std::sync::Arc;

use p2panda::VerifyingKey;
use tokio::sync::mpsc;

use crate::node::actor::Command;
use crate::stores::OpStore;
use crate::topic::TopicId;

/// What the node hands the router at startup.
pub struct LanRouterParams {
    pub enabled: bool,
    pub data_path: PathBuf,
    pub device_id: VerifyingKey,
    pub op_store: OpStore,
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

    use anyhow::{Context as _, anyhow, ensure};
    use dash_router::core::{LogRanges, Op, Ranges, Seq};
    use dash_router::{
        AsyncStorage, GossipPublisher, GossipSubscription, GossipTransport, PeerKey,
        WatchableStorage,
    };
    use futures::StreamExt as _;
    use p2panda::operation::{Header, LogId, Operation};
    use p2panda::streams::{EphemeralStreamPublisher, EphemeralStreamSubscription};
    use p2panda_core::Body;
    use serde::{Deserialize, Serialize};
    use serde_bytes::ByteBuf;
    use tokio::sync::{broadcast, oneshot};

    use crate::DeviceId;

    /// The router's log identity: `(LogId, author)`, prefix first so the
    /// relay store's ordered scans keep one topic's logs contiguous. The
    /// prefix (`LogId = blake3(topic)`) is what a subscription names.
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

        pub(crate) fn author(&self) -> anyhow::Result<VerifyingKey> {
            Ok(VerifyingKey::from_bytes(&self.author)?)
        }
    }

    impl std::fmt::Display for RouterLog {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}/{}",
                hex::encode(&self.log_id[..4]),
                hex::encode(&self.author[..4])
            )
        }
    }

    impl dash_router::core::Log for RouterLog {
        type Prefix = LogId;
        fn prefix(&self) -> LogId {
            self.log_id()
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
    #[allow(dead_code)] // until Task 6
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
    }

    #[allow(dead_code)] // until Task 6
    impl OpStoreExt {
        pub(crate) fn new(store: OpStore) -> Self {
            Self {
                store,
                topics: Default::default(),
                hints: broadcast::channel(64).0,
                rejected: Default::default(),
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

        pub(crate) fn has_topic(&self, log_id: &LogId) -> bool {
            self.topics.read().unwrap().contains_key(log_id)
        }

        pub(crate) fn hint(&self, log: RouterLog) {
            let _ = self.hints.send(BTreeSet::from([log])); // no receivers is fine
        }

        pub(crate) fn rejected(&self) -> u64 {
            self.rejected.load(Ordering::Relaxed)
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
            let Ok(author) = log.author() else {
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
                let Ok(author) = log.author() else {
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
                        *log,
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
                    header.verifying_key == log.author()?,
                    "header author != log author"
                );
                ensure!(
                    header.extensions.log_id() == log.log_id(),
                    "header log id != log"
                );
                let tx = self
                    .import_tx_of(&log.log_id())
                    .ok_or_else(|| anyhow!("no topic for log {log}"))?;
                let operation = Operation {
                    hash: header.hash(),
                    header,
                    body: op.payload.as_deref().map(Body::from_bytes),
                };
                tx.send(operation)
                    .await
                    .map_err(|_| anyhow!("import channel closed"))?;
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
    /// compile-time assert. Peers on a different network id never connect
    /// at all (every ALPN is hashed with it), so no further scoping.
    #[allow(dead_code)] // until Task 6
    pub(crate) fn router_topic() -> TopicId {
        p2panda::Hash::digest(dash_router::GOSSIP_TOPIC.as_bytes()).into()
    }

    #[allow(dead_code)] // until Task 6
    pub(crate) struct Publisher(EphemeralStreamPublisher<ByteBuf>);

    impl GossipPublisher for Publisher {
        async fn publish(&mut self, bytes: Vec<u8>) -> anyhow::Result<()> {
            self.0
                .publish(ByteBuf::from(bytes))
                .await
                .map_err(|e| anyhow!("gossip publish: {e}"))
        }
    }

    #[allow(dead_code)] // until Task 6
    pub(crate) struct Subscription(EphemeralStreamSubscription<ByteBuf>);

    impl GossipSubscription for Subscription {
        async fn next(&mut self) -> Option<(PeerKey, Vec<u8>)> {
            let msg = self.0.next().await?;
            Some((PeerKey::from(msg.author()), msg.body().to_vec()))
        }
    }

    #[allow(dead_code)] // until Task 6
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

    pub struct LanRouter;

    impl LanRouter {
        /// Task 6 replaces this body with the real spawn; until then the
        /// feature-on build behaves like the feature-off one.
        pub async fn spawn(params: LanRouterParams) -> anyhow::Result<Option<Arc<Self>>> {
            let _ = params;
            Ok(None)
        }
        pub async fn subscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
            Ok(())
        }
        pub async fn unsubscribe_topic(&self, _topic: TopicId) -> anyhow::Result<()> {
            Ok(())
        }
        pub fn hint_changed(&self, _author: VerifyingKey, _topic: TopicId) {}
        pub async fn shutdown(&self) {}
    }

    #[cfg(test)]
    mod router_log_tests {
        use super::*;
        use dash_router::core::Log;
        use dash_router::disk::LogKey;

        #[test]
        fn prefix_is_the_log_id_and_order_is_prefix_first() {
            // Seeds swapped from the brief's [1;32]/[2;32] assignment: the
            // derived verifying-key bytes for seed [1;32] are lexically
            // greater than for [2;32], the opposite of what the ordering
            // assertion below needs. Swapping which seed produces key_a vs
            // key_b keeps the assertion meaningful without relying on
            // ed25519 key-derivation internals.
            let key_a = p2panda::SigningKey::from_bytes(&[2; 32]).verifying_key();
            let key_b = p2panda::SigningKey::from_bytes(&[1; 32]).verifying_key();
            let t1 = TopicId::from([1u8; 32]);
            let t2 = TopicId::from([2u8; 32]);
            let l = RouterLog::new(LogId::from_topic(t1), key_b);
            assert_eq!(l.prefix(), LogId::from_topic(t1));
            assert_eq!(l.log_id(), LogId::from_topic(t1));
            assert_eq!(l.author().unwrap(), key_b);
            // Same topic, different authors, sorts inside the topic; a later
            // topic sorts after regardless of author bytes.
            let same_topic_other_author = RouterLog::new(LogId::from_topic(t1), key_a);
            let other_topic = RouterLog::new(LogId::from_topic(t2), key_a);
            assert!(same_topic_other_author < l);
            assert!(l < other_topic || other_topic < l);
            assert_eq!(l.prefix(), same_topic_other_author.prefix());
            assert_ne!(l.prefix(), other_topic.prefix());
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

        /// Review focus 3.
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
        }

        /// Review focus 2.
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

        /// Review focus 4.
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
            assert!(bogus.author().is_err(), "fixture needs an invalid key");
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
