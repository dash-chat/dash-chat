use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use dashchat_node::{Node, NodeConfig};

const DASHCHAT_MAILBOX_ID: &str = "dashchat-mailbox";

static NODES: Mutex<Option<HashMap<PathBuf, Node>>> = Mutex::new(None);

/// Build or retrieve a Node instance for the given data path.
///
/// If a Node has already been built for this path, returns the cached instance.
/// Otherwise creates a new Node with the production mailbox registered and caches it.
///
/// This ensures that `receive_push_notification` (which runs in an Android background service)
/// reuses the existing Node when the app is already running, avoiding duplicate SQLite connections.
pub async fn build_node(
    data_path: PathBuf,
    notification_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::Notification>>,
) -> anyhow::Result<Node> {
    // Fast path: return cached node if one exists for this path
    {
        let guard = NODES.lock().unwrap();
        if let Some(map) = guard.as_ref() {
            if let Some(node) = map.get(&data_path) {
                return Ok(node.clone());
            }
        }
    }

    // Slow path: create the node (no lock held across await)
    let config = if cfg!(feature = "e2e-tests") {
        let mut config = dashchat_node::NodeConfig::default();
        config.mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        config.mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        config
    } else {
        dashchat_node::NodeConfig::default()
    };
    let node = Node::new(data_path.clone(), config, notification_tx).await?;

    let mailbox_url = crate::mailbox::default_mailbox_url();
    let mailbox_client =
        mailbox_client::toy::ToyMailboxClient::new(DASHCHAT_MAILBOX_ID.to_string(), mailbox_url);
    node.mailboxes.register(mailbox_client).await;

    // Store it; if another thread raced us for the same path, use theirs
    {
        let mut guard = NODES.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(existing) = map.get(&data_path) {
            return Ok(existing.clone());
        }
        map.insert(data_path, node.clone());
    }

    Ok(node)
}
