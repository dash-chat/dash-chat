use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use dashchat_node::Node;

const DASHCHAT_MAILBOX_ID: &str = "dashchat-mailbox";

/// Cache for Node instances built when receiving a push notification in the background
/// Only to be reused when multiple notifications are processed sequentially
static NODES: Mutex<Option<HashMap<PathBuf, Node>>> = Mutex::new(None);

pub async fn build_node(
    data_path: PathBuf,
    notification_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::Notification>>,
    topic_subscribed_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::topic::TopicId>>,
) -> anyhow::Result<Node> {
    let config = if cfg!(feature = "e2e-tests") {
        let mut config = dashchat_node::NodeConfig::default();
        config.mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        config.mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        config
    } else {
        dashchat_node::NodeConfig::default()
    };
    let node = Node::new(data_path, config, notification_tx, topic_subscribed_tx).await?;

    let mailbox_url = crate::mailbox::default_mailbox_url();
    let mailbox_client =
        mailbox_client::toy::ToyMailboxClient::new(DASHCHAT_MAILBOX_ID.to_string(), mailbox_url);
    node.mailboxes.register(mailbox_client).await;

    Ok(node)
}

/// Get an existing cached Node, or create one without notification channels and cache it.
///
/// Used by `receive_push_notification` as a fallback when the app's managed state
/// is not available. Reuses a cached Node if one exists for this path (avoiding
/// duplicate SQLite connections), otherwise creates and caches a new one.
/// Cached nodes are cleared by `clear_cached_nodes` once `async_setup` manages
/// the authoritative Node.
pub async fn get_or_build_node(data_path: PathBuf) -> anyhow::Result<Node> {
    // Fast path: return cached node if the app is already running
    {
        let guard = NODES.lock().expect("NODES mutex poisoned");
        if let Some(map) = guard.as_ref() {
            if let Some(node) = map.get(&data_path) {
                return Ok(node.clone());
            }
        }
    }

    // App is not running — create a node without channels and cache it
    let node = build_node(data_path.clone(), None, None).await?;

    let mut guard = NODES.lock().expect("NODES mutex poisoned");
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(data_path, node.clone());

    Ok(node)
}

/// Drop all cached nodes. Called by `async_setup` after the authoritative Node
/// (with notification channels) has been managed by Tauri.
pub fn clear_cached_nodes() {
    let mut guard = NODES.lock().expect("NODES mutex poisoned");
    *guard = None;
}
