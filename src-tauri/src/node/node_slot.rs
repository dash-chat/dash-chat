use std::path::PathBuf;
use std::sync::LazyLock;

use async_rx::StreamExt as _;
use dashchat_node::Node;
use futures::{Stream, StreamExt};
use tokio::sync::{watch, Mutex};
use tokio_stream::wrappers::WatchStream;

use crate::node::app_node::AppNode;
use crate::node::node_context::{NodeContext, NodeRole};

/// The single Node owned by this process, together with the context it was
/// built for.
///
/// Reused across sequential push notifications, and also holds the main app
/// Node when the app is running. A watch, so long-lived subscriptions follow the
/// node across swaps.
static SLOT: LazyLock<watch::Sender<Option<AppNode>>> = LazyLock::new(|| watch::channel(None).0);

/// Serializes every slot mutation that must not interleave: node builds and
/// evictions ([`get_or_build_node`]) and teardown ([`clear`]). Holding it across
/// a build stops two pushes racing in the same extension process from opening two
/// SQLite pools on the same database; holding it across `clear` stops a
/// background teardown from racing an in-flight foreground build (which would
/// leave the caller believing it resumed a node that was just torn down).
/// Deliberately separate from `SLOT`: the cache lookup must never block behind an
/// in-flight build, or a slow `build_node` + mailbox handshake starves every
/// other push and they all miss iOS's ~30s budget.
static LIFECYCLE_LOCK: Mutex<()> = Mutex::const_new(());

/// The stream `make` builds on the app's node, switched to the next node after
/// every swap, and silent while there is none.
///
/// The old node's stream is dropped once the subscription sees the swap, which
/// may be after teardown has started; `Node::shutdown` closes its pools itself,
/// so a lingering stream holds no file lock (0xdead10cc).
pub(crate) fn node_stream<S, F>(make: F) -> impl Stream<Item = S::Item> + Send + 'static
where
    F: Fn(Node) -> S + Send + 'static,
    S: Stream + Send + 'static,
{
    WatchStream::new(SLOT.subscribe())
        .map(move |app_node| {
            let node = app_node
                .filter(|app_node| app_node.context.role.can_be_used_for(NodeRole::App))
                .map(|app_node| make(app_node.node));
            futures::stream::iter(node).flatten()
        })
        .fuse()
        .switch()
}

pub(crate) async fn current_node() -> Option<AppNode> {
    SLOT.borrow().clone()
}

/// The slot's Node, but only if its role can satisfy `role`. Side-effect free:
/// never builds, evicts, or registers anything, so it is safe on hot read paths
/// (e.g. per-command `AppNodeManager::get`).
pub(crate) async fn current_node_for_role(role: NodeRole) -> Option<Node> {
    let app_node = current_node().await?;
    app_node
        .context
        .role
        .can_be_used_for(role)
        .then_some(app_node.node)
}

/// The result of acquiring a Node from the slot.
pub struct AcquiredNode {
    /// The acquired Node.
    pub node: Node,
    /// Whether the Node was newly built for this request (as opposed to reused
    /// from the slot).
    pub is_new: bool,
}

/// Get a Node for handling a push notification.
///
/// Resolution order:
/// 1. The app's managed state (authoritative Node with notification channels).
///    When found, the cache is cleared since it's no longer needed.
/// 2. A previously cached Node with a compatible context.
/// 3. Build a new Node for the requested context and cache it.
#[cfg_attr(not(mobile), allow(dead_code))]
pub async fn get_node_for_push_notification(
    data_path: &PathBuf,
    context: NodeContext,
) -> anyhow::Result<AcquiredNode> {
    let acquired = get_or_build_node(data_path, context).await?;

    // A reused node read the stored topics when it was built, hours ago: topics
    // subscribed since then are absent from its mailbox subscriptions, so their
    // operations are never polled and their pushes degrade to the generic
    // notification. A freshly built node has just read them.
    if !acquired.is_new {
        if let Err(err) = acquired.node.resync().await {
            log::warn!("failed to resync stored topics in push extension: {err:?}");
        }
    }

    Ok(acquired)
}

/// Get a Node
///
/// Resolution order:
/// 1. A previously cached Node with a compatible context.
/// 2. Build a new Node for the requested context and cache it.
pub async fn get_or_build_node(
    data_path: &PathBuf,
    context: NodeContext,
) -> anyhow::Result<AcquiredNode> {
    // Fast path: return a cached node with a compatible context without
    // blocking on any in-flight build.
    if let Some(app_node) = current_node().await {
        if app_node.is_compatible_with(&context) {
            return Ok(AcquiredNode {
                node: app_node.node,
                is_new: false,
            });
        }
    }

    // Serialize the build (one SQLite pool per DB) — but never hold the cache
    // lock across it. A push that arrives mid-build waits here, then finds the
    // just-built node on this re-check instead of building a second one.
    let _lifecycle_guard = LIFECYCLE_LOCK.lock().await;
    if let Some(app_node) = current_node().await {
        if app_node.is_compatible_with(&context) {
            return Ok(AcquiredNode {
                node: app_node.node,
                is_new: false,
            });
        }
    }

    // A Node built for a different context is already in the slot. Evict and
    // shut it down before building the new one so two Nodes are never alive at
    // the same time in this process. The slot is left empty during shutdown and
    // build so a concurrent caller cannot observe a Node whose context disagrees
    // with its actual behavior.
    if let Some(old_app_node) = SLOT.send_replace(None) {
        old_app_node.teardown().await;
    }

    log::info!("No compatible node in the cache, building node from scratch.");

    let node = Node::new(
        data_path.clone(),
        context.node_config(),
        context.notification_tx.clone(),
        context.topic_subscribed_tx.clone(),
    )
    .await?;

    let app_node = AppNode::new(context, node.clone())?;
    SLOT.send_replace(Some(app_node));

    Ok(AcquiredNode { node, is_new: true })
}

/// Remove the cached node from the slot and tear it down, aborting app-specific
/// tasks and shutting the Node down exactly once. Holds [`LIFECYCLE_LOCK`] across
/// the teardown so it cannot interleave with an in-flight build. Returns whether
/// a node was actually torn down.
pub async fn clear() -> bool {
    let _lifecycle_guard = LIFECYCLE_LOCK.lock().await;
    let app_node = SLOT.send_replace(None);
    match app_node {
        Some(app_node) => {
            app_node.teardown().await;
            true
        }
        None => false,
    }
}
