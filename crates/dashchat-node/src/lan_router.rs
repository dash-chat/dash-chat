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
    pub actor_tx: mpsc::Sender<Command>,
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
}
