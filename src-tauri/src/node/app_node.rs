use std::sync::Arc;

use dashchat_node::Node;
use tokio_util::task::AbortOnDropHandle;

use crate::node::node_context::NodeContext;

/// A Node that is owned by this process, together with the context it was
/// built for.
#[derive(Clone)]
pub struct AppNode {
    /// The context that describes this Node's role and channels.
    pub context: NodeContext,
    /// The Node itself.
    pub node: Node,
    /// Local-mailbox mDNS discovery task for app Nodes. Wrapped in `Arc` so the
    /// handle can be shared across clones of this `AppNode`; the discovery task
    /// is aborted when the last reference is dropped.
    pub mdns_discovery: Option<Arc<AbortOnDropHandle<()>>>,
}

impl AppNode {
    /// Create a new `AppNode` from the given context and Node. Spawns
    /// app-specific tasks (like local-mailbox mDNS discovery) when enabled by
    /// the context.
    pub fn new(context: NodeContext, node: Node) -> anyhow::Result<Self> {
        let mdns_discovery = match context.app_handle.as_ref() {
            Some(app) if context.enable_mdns_mailbox() => Some(Arc::new(
                crate::mailbox::spawn_local_mailbox_mdns_discovery(app, node.clone())?,
            )),
            None if context.enable_mdns_mailbox() => {
                log::error!(
                    "enable_mdns_mailbox is true but app_handle is missing; skipping mDNS discovery"
                );
                None
            }
            _ => None,
        };

        Ok(Self {
            context,
            node,
            mdns_discovery,
        })
    }

    /// Whether this Node can be reused to satisfy a request for the given
    /// context.
    pub fn is_compatible_with(&self, context: &NodeContext) -> bool {
        self.context.is_compatible_with(context)
    }

    /// Abort app-specific tasks and shut the Node down.
    pub async fn teardown(self) {
        if let Some(discovery) = self.mdns_discovery {
            discovery.abort();
        }
        if let Err(err) = self.node.shutdown().await {
            log::error!("Failed to shut down node: {err:?}");
        }
    }
}
