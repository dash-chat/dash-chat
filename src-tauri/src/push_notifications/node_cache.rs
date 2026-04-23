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
pub async fn get_node(data_path: PathBuf) -> anyhow::Result<Node> {
    // Try the app's managed state first
    if let Some(handle) = crate::APP_HANDLE.get() {
        if let Some(node) = handle.try_state::<Node>().map(|s| s.inner().clone()) {
            // The app is fully running — clear any stale cached nodes
            clear().await;
            return Ok(node);
        }
    }

    // Fall back to the cache, or build and cache a new node
    let mut guard = NODES.lock().await;
    let map = guard.get_or_insert_with(HashMap::new);

    if let Some(node) = map.get(&data_path) {
        return Ok(node.clone());
    }

    let node = crate::setup::build_node(data_path.clone(), None, None).await?;
    map.insert(data_path, node.clone());

    Ok(node)
}

/// Drop all cached nodes.
pub async fn clear() {
    let mut guard = NODES.lock().await;
    *guard = None;
}
