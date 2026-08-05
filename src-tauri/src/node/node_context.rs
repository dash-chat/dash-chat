use tokio::sync::mpsc;

/// The capabilities and wiring with which a Node is built.
///
/// A `NodeContext` describes what a Node is allowed to do and which external
/// channels it participates in. Callers build a Node by supplying a context, and
/// different contexts can enforce different constraints (for example, disabling
/// peer-to-peer networking when running in an extension that shares an identity
/// with another process).
#[derive(Clone)]
pub struct NodeContext {
    /// Whether peer-to-peer discovery and relay are enabled.
    pub p2p_enabled: bool,
    /// Whether blob (media) sync is enabled.
    pub blob_sync_enabled: bool,
    /// Channel for forwarding node notifications to the app (webview + system
    /// notifications). None when running outside the main app process.
    pub notification_tx: Option<mpsc::Sender<dashchat_node::Notification>>,
    /// Channel for tracking topic subscriptions for push notifications. None when
    /// push setup is not available in this context.
    pub topic_subscribed_tx: Option<mpsc::Sender<dashchat_node::topic::TopicId>>,
}

impl NodeContext {
    /// Context used when handling push-notifications in a limited time window with the app closed:
    /// no P2P, no blob sync, and no app-lifetime channels.
    pub fn for_push_notifications() -> Self {
        Self {
            p2p_enabled: false,
            blob_sync_enabled: false,
            notification_tx: None,
            topic_subscribed_tx: None,
        }
    }

    /// Context used when the app is running in the foreground (or resuming from
    /// background on iOS): full networking and notification channels enabled.
    pub fn for_app(
        notification_tx: mpsc::Sender<dashchat_node::Notification>,
        topic_subscribed_tx: Option<mpsc::Sender<dashchat_node::topic::TopicId>>,
    ) -> Self {
        Self {
            p2p_enabled: true,
            blob_sync_enabled: true,
            notification_tx: Some(notification_tx),
            topic_subscribed_tx,
        }
    }

    /// Build a [`dashchat_node::NodeConfig`] from this context.
    pub fn node_config(&self) -> dashchat_node::NodeConfig {
        let mut config = if cfg!(feature = "e2e-tests") {
            let mut config = dashchat_node::NodeConfig::default();
            config.mdns_mode = p2panda::network::MdnsDiscoveryMode::Disabled;
            config
        } else {
            dashchat_node::NodeConfig::default()
        };

        if !self.p2p_enabled || std::env::var_os("DASHCHAT_NO_P2P").is_some() {
            config = config.no_p2p();

            if self.p2p_enabled {
                // Dev/testing escape hatch: force all communication through mailbox
                // servers so peers can't sync directly over p2p. Keeps blob sync so
                // media still flows over the mailbox.
                log::warn!("DASHCHAT_NO_P2P set: disabling peer-to-peer connectivity");
            }
        }

        if !self.blob_sync_enabled {
            config = config.no_blob_sync();
        }

        config
    }
}
