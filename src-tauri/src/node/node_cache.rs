use std::collections::HashMap;
use std::path::PathBuf;

use dashchat_node::Node;
use tauri::Manager;
use tokio::sync::Mutex;

/// Cache for Node instances built when receiving a push notification in the background.
/// Only to be reused when multiple notifications are processed sequentially.
static NODES: Mutex<Option<HashMap<PathBuf, Node>>> = Mutex::const_new(None);

/// Serializes the (slow) node build so two pushes racing in the same extension
/// process don't open two SQLite pools on the same database. Deliberately
/// separate from `NODES`: the cache lookup must never block behind an in-flight
/// build, or a slow `build_node` + mailbox handshake starves every other push and
/// they all miss iOS's ~30s budget.
static BUILD_LOCK: Mutex<()> = Mutex::const_new(());

async fn cached_node(data_path: &PathBuf) -> Option<Node> {
    NODES
        .lock()
        .await
        .as_ref()
        .and_then(|map| map.get(data_path).cloned())
}

/// Get a Node for handling a push notification.
///
/// Resolution order:
/// 1. The app's managed state (authoritative Node with notification channels).
///    When found, the cache is cleared since it's no longer needed.
/// 2. A previously cached Node for this data path.
/// 3. Build a new Node without channels and cache it.
pub async fn get_node(data_path: &PathBuf) -> anyhow::Result<Node> {
    // Try the app's managed state first. If the app is running but its node is
    // paused (backgrounded on iOS), fall through and build the extension's own
    // node so we never hold two live p2p endpoints on the shared identity.
    if let Some(handle) = crate::APP_HANDLE.get() {
        if let Some(app_node) = handle.try_state::<crate::node::AppNode>() {
            if let Ok(node) = app_node.get().await {
                // The app is fully running — clear any stale cached nodes
                clear().await;
                log::info!("The app is opened: reuse the currently running node.");
                return Ok(node);
            }
        }
    }

    // Fast path: return a cached node without blocking on any in-flight build.
    if let Some(node) = cached_node(data_path).await {
        return Ok(node);
    }

    // Serialize the build (one SQLite pool per DB) — but never hold the cache
    // lock across it. A push that arrives mid-build waits here, then finds the
    // just-built node on this re-check instead of building a second one.
    let _build_guard = BUILD_LOCK.lock().await;
    if let Some(node) = cached_node(data_path).await {
        return Ok(node);
    }

    log::info!("No nodes in the cache, building node from scratch.");

    let node = Node::new(
        data_path.clone(),
        crate::node::AppNode::node_config(true),
        None,
        None,
    )
    .await?;

    // Best-effort: the extension only runs when a push arrives (network present),
    // so resolve and track the cloud mailbox once so the sync below can fetch.
    // Only track it as a fetch source — do NOT register ourselves back as a blob
    // source here: `register_cloud_mailbox`'s up-to-10s `wait_endpoint_online`
    // would eat the extension's ~30s budget before the operation poll can start,
    // making iOS kill the extension and deliver the raw APNS fallback.
    if let Err(err) = crate::setup::track_cloud_mailbox(&node).await {
        log::warn!("failed to track cloud mailbox in push extension: {err:?}");
    }

    NODES
        .lock()
        .await
        .get_or_insert_with(HashMap::new)
        .insert(data_path.clone(), node.clone());

    Ok(node)
}

/// Drop all cached nodes.
pub async fn clear() {
    let mut guard = NODES.lock().await;
    *guard = None;
}
