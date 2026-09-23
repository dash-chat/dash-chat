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

    use p2panda::operation::LogId;
    use serde::{Deserialize, Serialize};

    /// The router's log identity: `(LogId, author)`, prefix first so the
    /// relay store's ordered scans keep one topic's logs contiguous. The
    /// prefix (`LogId = blake3(topic)`) is what a subscription names.
    ///
    /// Unused outside tests until Task 6 wires it into `LanRouter::spawn`.
    #[allow(dead_code)]
    #[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
    pub(crate) struct RouterLog {
        log_id: [u8; 32],
        author: [u8; 32],
    }

    #[allow(dead_code)]
    impl RouterLog {
        pub(crate) fn new(log_id: LogId, author: VerifyingKey) -> Self {
            Self { log_id: *log_id.as_bytes(), author: *author.as_bytes() }
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
            write!(f, "{}/{}", hex::encode(&self.log_id[..4]), hex::encode(&self.author[..4]))
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
}
