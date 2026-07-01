use std::collections::HashMap;
use std::path::PathBuf;

use dashchat_node::Node;
use tauri::Manager;
use tokio::sync::Mutex;

/// Cache for Node instances built when receiving a push notification in the background.
/// Only to be reused when multiple notifications are processed sequentially.
static NODES: Mutex<Option<HashMap<PathBuf, Node>>> = Mutex::const_new(None);

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
        if let Some(app_node) = handle.try_state::<crate::app_node::AppNode>() {
            if let Ok(node) = app_node.get().await {
                // The app is fully running — clear any stale cached nodes
                clear().await;
                log::info!("The app is opened: reuse the currently running node.");
                return Ok(node);
            }
        }
    }

    // Fall back to the cache, or build and cache a new node
    let mut guard = NODES.lock().await;
    let map = guard.get_or_insert_with(HashMap::new);

    if let Some(node) = map.get(data_path) {
        return Ok(node.clone());
    }

    log::info!("No nodes in the cache, building node from scratch.");

    let node = Node::new(
        data_path.clone(),
        crate::app_node::AppNode::node_config(true),
        None,
        None,
    )
    .await?;
    // Best-effort: the extension only runs when a push arrives (network present),
    // so resolve and register the cloud mailbox once so the sync below can fetch.
    if let Err(err) = crate::app_node::register_cloud_mailbox(&node).await {
        log::warn!("failed to register cloud mailbox in push extension: {err:?}");
    }
    map.insert(data_path.clone(), node.clone());

    Ok(node)
}

/// Drop all cached nodes.
pub async fn clear() {
    let mut guard = NODES.lock().await;
    *guard = None;
}
