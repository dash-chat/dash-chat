use std::sync::Arc;

#[cfg(mobile)]
use dashchat_node::topic::TopicId;
use dashchat_node::Node;
#[cfg(mobile)]
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::{AbortOnDropHandle, TaskTracker};

use crate::node::node_context::NodeContext;
#[cfg(mobile)]
use crate::notifications::push_notifications::{
    push_notifications_url, SubscribeToPushNotificationsForTopicsTask,
};
#[cfg(mobile)]
use push_notifications_client::client::PushNotificationsClient;

/// The cloud-mailbox registration retry, held so it can be cancelled and drained
/// before the Node is shut down: it holds a `Node` clone and touches SQLite
/// pools, so it must stop before shutdown. Only the main app runs this.
#[derive(Clone)]
struct CloudMailboxRegistration {
    tracker: TaskTracker,
    token: CancellationToken,
}

impl CloudMailboxRegistration {
    /// Spawn the registration retry against `node`.
    fn spawn(node: Node) -> Self {
        let tracker = TaskTracker::new();
        let token = CancellationToken::new();
        tracker.spawn(
            token
                .clone()
                .run_until_cancelled_owned(dashchat_utils::retry_with_backoff(
                    None,
                    std::time::Duration::from_secs(2),
                    std::time::Duration::from_secs(10),
                    "register cloud mailbox",
                    move || {
                        let node = node.clone();
                        async move { crate::setup::register_cloud_mailbox(&node).await }
                    },
                )),
        );
        Self { tracker, token }
    }

    /// Cancel the retry and wait for it to drain.
    async fn shutdown(self) {
        self.token.cancel();
        self.tracker.close();
        self.tracker.wait().await;
    }
}

/// A Node that is owned by this process, together with the context it was
/// built for.
#[derive(Clone)]
pub struct AppNode {
    /// The context that describes this Node's role and channels.
    pub context: NodeContext,
    /// The Node itself.
    pub node: Node,
    /// Local-mailbox mDNS discovery task; aborted when the last clone drops it.
    mdns_discovery: Option<Arc<AbortOnDropHandle<()>>>,
    /// Cloud-mailbox registration retry. `None` when the context does not run it
    /// (only the main app does).
    registration: Option<CloudMailboxRegistration>,
    #[cfg(mobile)]
    pub(crate) push_notifications_topic_subscriptions: SubscribeToPushNotificationsForTopicsTask,
}

impl AppNode {
    /// Create a new `AppNode` from the given context and Node. Spawns
    /// app-specific tasks (like local-mailbox mDNS discovery) when enabled by
    /// the context. `subscribed_topics` receives every topic the Node subscribes
    /// to.
    pub fn new(
        context: NodeContext,
        node: Node,
        #[cfg(mobile)] subscribed_topics: mpsc::Receiver<TopicId>,
    ) -> anyhow::Result<Self> {
        let mdns_discovery = context
            .enable_mdns_mailbox()
            .then(|| crate::mailbox::spawn_local_mailbox_mdns_discovery(node.clone()))
            .transpose()?;

        let registration = context
            .enable_cloud_mailbox_registration()
            .then(|| CloudMailboxRegistration::spawn(node.clone()));

        #[cfg(mobile)]
        let push_notifications_topic_subscriptions =
            SubscribeToPushNotificationsForTopicsTask::spawn(
                node.clone(),
                context.app_handle.clone(),
                PushNotificationsClient::new(push_notifications_url())?,
                subscribed_topics,
            );

        Ok(Self {
            context,
            node,
            mdns_discovery: mdns_discovery.map(Arc::new),
            registration,
            #[cfg(mobile)]
            push_notifications_topic_subscriptions,
        })
    }

    /// Whether this Node can be reused to satisfy a request for the given
    /// context.
    pub fn is_compatible_with(&self, context: &NodeContext) -> bool {
        self.context.is_compatible_with(context)
    }

    /// Abort app-specific tasks and shut the Node down.
    pub async fn teardown(self) {
        #[cfg(mobile)]
        self.push_notifications_topic_subscriptions.shutdown().await;
        // Drain the cloud-mailbox retry first: it holds a `Node` clone and
        // touches SQLite pools, so it must stop before shutdown.
        if let Some(registration) = self.registration {
            registration.shutdown().await;
        }
        if let Some(discovery) = self.mdns_discovery {
            discovery.abort();
        }
        if let Err(err) = self.node.shutdown().await {
            log::error!("Failed to shut down node: {err:?}");
        }
    }
}
