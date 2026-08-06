use std::path::PathBuf;

use dashchat_node::Node;
use tauri::Manager;
use tokio::sync::Mutex;

use crate::node::app_node::AppNode;
use crate::node::node_context::NodeContext;

/// The single Node owned by this process, together with the context it was
/// built for.
///
/// Reused across sequential push notifications, and also holds the main app
/// Node when the app is running.
static SLOT: Mutex<Option<AppNode>> = Mutex::const_new(None);

/// Serializes the (slow) node build so two pushes racing in the same extension
/// process don't open two SQLite pools on the same database. Deliberately
/// separate from `SLOT`: the cache lookup must never block behind an in-flight
/// build, or a slow `build_node` + mailbox handshake starves every other push and
/// they all miss iOS's ~30s budget.
static BUILD_LOCK: Mutex<()> = Mutex::const_new(());

async fn current_node() -> Option<AppNode> {
    SLOT.lock().await.clone()
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
pub async fn get_node_for_push_notification(
    data_path: &PathBuf,
    context: NodeContext,
) -> anyhow::Result<AcquiredNode> {
    // Try the app's managed state first. If the app is running but its node is
    // paused (backgrounded on iOS), fall through and build the extension's own
    // node so we never hold two live p2p endpoints on the shared identity.
    if let Some(handle) = crate::APP_HANDLE.get() {
        if let Some(app_node_manager) = handle.try_state::<crate::node::AppNodeManager>() {
            if let Ok(node) = app_node_manager.get().await {
                // The app is fully running — clear any stale cached node
                clear().await;
                log::info!("The app is opened: reuse the currently running node.");
                return Ok(AcquiredNode {
                    node,
                    is_new: false,
                });
            }
        }
    }

    let acquired = get_or_build_node(data_path, context).await?;

    // Best-effort: when we just built a fresh node, resolve and track the cloud
    // mailbox once so the sync below can fetch. Cached nodes already did this
    // when they were first built. Track it as a fetch source only — do NOT
    // register ourselves back as a blob source here: `register_cloud_mailbox`'s
    // up-to-10s `wait_endpoint_online` would eat the extension's ~30s budget
    // before the operation poll can start, making iOS kill the extension and
    // deliver the raw APNS fallback.
    if acquired.is_new {
        if let Err(err) = crate::setup::track_cloud_mailbox(&acquired.node).await {
            log::warn!("failed to track cloud mailbox in push extension: {err:?}");
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
    let _build_guard = BUILD_LOCK.lock().await;
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
    let maybe_old_node = SLOT.lock().await.take().map(|app_node| app_node.node);
    if let Some(old_node) = maybe_old_node {
        if let Err(err) = old_node.shutdown().await {
            log::warn!("failed to shut down evicted node: {err:?}");
        }
    }

    log::info!("No compatible node in the cache, building node from scratch.");

    let node = Node::new(
        data_path.clone(),
        context.node_config(),
        context.notification_tx.clone(),
        context.topic_subscribed_tx.clone(),
    )
    .await?;

    *SLOT.lock().await = Some(AppNode::new(context, node.clone()));

    Ok(AcquiredNode { node, is_new: true })
}

/// Drop the cached node.
pub async fn clear() {
    *SLOT.lock().await = None;
}
