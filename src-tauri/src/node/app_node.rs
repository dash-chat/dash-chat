use std::sync::{Arc, Mutex};

use dashchat_node::Node;
use tokio_util::sync::CancellationToken;
use tokio_util::task::{AbortOnDropHandle, TaskTracker};

use crate::node::node_context::NodeContext;

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
    /// Local-mailbox mDNS discovery task. Replaceable so it can be re-armed;
    /// the old task is aborted when the handle is replaced or dropped.
    pub mdns_discovery: Arc<Mutex<Option<AbortOnDropHandle<()>>>>,
    /// Cloud-mailbox registration retry. `None` when the context does not run it
    /// (only the main app does).
    registration: Option<CloudMailboxRegistration>,
}

impl AppNode {
    /// Create a new `AppNode` from the given context and Node. Spawns
    /// app-specific tasks (like local-mailbox mDNS discovery) when enabled by
    /// the context.
    pub fn new(context: NodeContext, node: Node) -> anyhow::Result<Self> {
        let mdns_discovery = match context.app_handle.as_ref() {
            Some(app) if context.enable_mdns_mailbox() => Some(
                crate::mailbox::spawn_local_mailbox_mdns_discovery(app, node.clone())?,
            ),
            None if context.enable_mdns_mailbox() => {
                log::error!(
                    "enable_mdns_mailbox is true but app_handle is missing; skipping mDNS discovery"
                );
                None
            }
            _ => None,
        };

        let registration = context
            .enable_cloud_mailbox_registration()
            .then(|| CloudMailboxRegistration::spawn(node.clone()));

        Ok(Self {
            context,
            node,
            mdns_discovery: Arc::new(Mutex::new(mdns_discovery)),
            registration,
        })
    }

    /// Re-issue the local-mailbox mDNS browse, so a hub that announced while the
    /// app was backgrounded is found now rather than up to an hour later.
    pub fn rearm_mdns_discovery(&self) {
        let Some(app) = self.context.app_handle.as_ref() else {
            return;
        };
        if !self.context.enable_mdns_mailbox() {
            return;
        }
        match crate::mailbox::spawn_local_mailbox_mdns_discovery(app, self.node.clone()) {
            Ok(task) => *self.mdns_discovery.lock().unwrap() = Some(task),
            Err(err) => log::warn!("Failed to re-arm local mailbox mdns discovery: {err:?}"),
        }
    }

    /// Whether this Node can be reused to satisfy a request for the given
    /// context.
    pub fn is_compatible_with(&self, context: &NodeContext) -> bool {
        self.context.is_compatible_with(context)
    }

    /// Abort app-specific tasks and shut the Node down.
    pub async fn teardown(self) {
        // Drain the cloud-mailbox retry first: it holds a `Node` clone and
        // touches SQLite pools, so it must stop before shutdown.
        if let Some(registration) = self.registration {
            registration.shutdown().await;
        }
        if let Some(discovery) = self.mdns_discovery.lock().unwrap().take() {
            discovery.abort();
        }
        if let Err(err) = self.node.shutdown().await {
            log::error!("Failed to shut down node: {err:?}");
        }
    }
}
