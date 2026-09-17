use tauri::AppHandle;
use tokio::sync::mpsc;

/// The role a Node is playing in the current process.
///
/// Roles determine both the capabilities a Node is built with and whether a
/// Node built for one role can be reused to satisfy a request for another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NodeRole {
    /// The main app is running with full networking and notification channels.
    App,
    /// A background task is running, possibly with reduced capabilities.
    BackgroundTask,
    /// A push notification is being handled in a limited time window, typically
    /// without P2P or blob sync.
    PushNotification,
}

impl NodeRole {
    /// Whether peer-to-peer discovery and relay are enabled for this role.
    pub fn p2p_enabled(&self) -> bool {
        match self {
            Self::App => true,
            Self::BackgroundTask | Self::PushNotification => false,
        }
    }

    /// Whether blob (media) sync is enabled for this role.
    pub fn blob_sync_enabled(&self) -> bool {
        match self {
            Self::App => true,
            Self::BackgroundTask | Self::PushNotification => false,
        }
    }

    /// Whether a Node built for this role can be used to satisfy a request for
    /// the `requested` role.
    pub fn can_be_used_for(&self, requested: Self) -> bool {
        if *self == requested {
            return true;
        }
        match (*self, requested) {
            // A full app Node can satisfy push handling.
            (Self::App, Self::PushNotification) => true,
            (Self::App, Self::BackgroundTask) => true,
            _ => false,
        }
    }
}

/// The p2p network e2e agents live on. An e2e build is given the run's
/// `E2E_NETWORK_ID` (64 hex characters), so test runs sharing a LAN, each
/// built with its own id, refuse each other's peers; a build without one gets
/// the id every such build shares.
fn e2e_network_id() -> [u8; 32] {
    match option_env!("E2E_NETWORK_ID") {
        Some(id) => {
            <[u8; 32] as hex::FromHex>::from_hex(id).expect("E2E_NETWORK_ID is 64 hex characters")
        }
        None => *b"dashchat end-to-end test network",
    }
}

/// The capabilities and wiring with which a Node is built.
///
/// A `NodeContext` describes what a Node is allowed to do and which external
/// channels it participates in. The concrete capabilities are determined by the
/// [`NodeRole`]; callers supply the role and any role-specific channels.
#[derive(Clone)]
pub struct NodeContext {
    /// The role this Node is playing.
    pub role: NodeRole,
    /// Channel for forwarding node notifications to the app (webview + system
    /// notifications). None when running outside the main app process.
    pub notification_tx: Option<mpsc::Sender<dashchat_node::Notification>>,
    /// Channel for tracking topic subscriptions for push notifications. None when
    /// push setup is not available in this context.
    pub topic_subscribed_tx: Option<mpsc::Sender<dashchat_node::topic::TopicId>>,
    /// The Tauri app handle, available only when the Node is built for the main
    /// app process (used to spawn app-lifetime tasks like local-mailbox mDNS
    /// discovery).
    pub app_handle: Option<AppHandle>,
}

impl NodeContext {
    /// Context used when handling push-notifications in a limited time window with the app closed:
    /// no P2P, no blob sync, and no app-lifetime channels.
    #[cfg_attr(not(mobile), allow(dead_code))]
    pub fn for_push_notifications() -> Self {
        Self {
            role: NodeRole::PushNotification,
            notification_tx: None,
            topic_subscribed_tx: None,
            app_handle: None,
        }
    }

    /// Context used when a long-lived background service is keeping a Node alive
    /// without the main app running: no P2P, no blob sync, and no app-lifetime
    /// channels.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn for_background_task() -> Self {
        Self {
            role: NodeRole::BackgroundTask,
            notification_tx: None,
            topic_subscribed_tx: None,
            app_handle: None,
        }
    }

    /// Context used when the app is running in the foreground (or resuming from
    /// background on iOS): full networking and notification channels enabled.
    pub fn for_app(
        app: &AppHandle,
        notification_tx: mpsc::Sender<dashchat_node::Notification>,
        topic_subscribed_tx: Option<mpsc::Sender<dashchat_node::topic::TopicId>>,
    ) -> Self {
        Self {
            role: NodeRole::App,
            notification_tx: Some(notification_tx),
            topic_subscribed_tx,
            app_handle: Some(app.clone()),
        }
    }

    /// Whether local-mailbox mDNS discovery should be enabled for a Node built
    /// in this context.
    pub fn enable_mdns_mailbox(&self) -> bool {
        self.role == NodeRole::App && self.app_handle.is_some()
    }

    /// Whether the cloud-mailbox registration retry should run for a Node built
    /// in this context. Only the main app registers itself as a blob source; the
    /// push extension merely tracks the mailbox as a fetch source.
    pub fn enable_cloud_mailbox_registration(&self) -> bool {
        self.role == NodeRole::App
    }

    /// Whether a Node built for this context can be reused to satisfy a request
    /// for the `requested` context.
    pub fn is_compatible_with(&self, requested: &Self) -> bool {
        self.role.can_be_used_for(requested.role)
    }

    /// Whether a Node built in this context takes part in peer-to-peer
    /// connectivity: never for the push and background roles; for the app,
    /// whatever the persisted `p2p_enabled` setting says.
    fn p2p_enabled(&self) -> bool {
        self.role.p2p_enabled()
            && self
                .app_handle
                .as_ref()
                .is_none_or(crate::settings::load_p2p_enabled)
    }

    /// Build a [`dashchat_node::NodeConfig`] from this context.
    pub fn node_config(&self) -> dashchat_node::NodeConfig {
        let mut config = if cfg!(feature = "e2e-tests") {
            let mut config = dashchat_node::NodeConfig::default();
            // Distinct network id so e2e agents with mDNS discovery active can't
            // cross-talk with production/dev instances on the same LAN: every
            // ALPN is hashed with the network id, so foreign connections are
            // rejected at protocol negotiation.
            config.network_id = e2e_network_id();
            config.message_ack_debounce = std::time::Duration::from_millis(300);
            config
        } else {
            dashchat_node::NodeConfig::default()
        };

        // The push extension's short-lived background node only reads
        // operations to build notifications; it must not author any.
        config.enable_message_acks = self.role == NodeRole::App;

        if !self.p2p_enabled() {
            config = config.no_p2p();
        }

        if !self.role.blob_sync_enabled() {
            config = config.no_blob_sync();
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeContext, NodeRole};

    fn app_context() -> NodeContext {
        NodeContext {
            role: NodeRole::App,
            notification_tx: None,
            topic_subscribed_tx: None,
            app_handle: None,
        }
    }

    #[test]
    fn p2p_follows_the_role() {
        assert!(app_context().node_config().enable_p2p);
        assert!(
            !NodeContext::for_push_notifications()
                .node_config()
                .enable_p2p
        );
    }

    // Underpins the removal of the startup `node_slot::clear()`: because a
    // push-role node cannot satisfy an App request, `get_or_build_node(App)`
    // evicts a stale push node from the slot before building the app node, so
    // no separate startup teardown is needed.
    #[test]
    fn push_node_cannot_satisfy_app_request() {
        assert!(!NodeRole::PushNotification.can_be_used_for(NodeRole::App));
    }

    #[test]
    fn app_node_satisfies_push_and_app_requests() {
        assert!(NodeRole::App.can_be_used_for(NodeRole::App));
        assert!(NodeRole::App.can_be_used_for(NodeRole::PushNotification));
    }
}
