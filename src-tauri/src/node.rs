use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use dashchat_node::Node;

const DASHCHAT_MAILBOX_ID: &str = "dashchat-mailbox";

static NODES: Mutex<Option<HashMap<PathBuf, Node>>> = Mutex::new(None);

async fn build_node(
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

/// Build a Node for app startup with notification channels, and cache it.
///
/// Always creates a fresh Node (replacing any previously cached one for this path),
/// ensuring the notification channels are properly wired up.
pub async fn build_and_cache_node(
    data_path: PathBuf,
    notification_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::Notification>>,
    topic_subscribed_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::topic::TopicId>>,
) -> anyhow::Result<Node> {
    let node = build_node(data_path.clone(), notification_tx, topic_subscribed_tx).await?;

    let mut guard = NODES.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(data_path, node.clone());

    Ok(node)
}

/// Get an existing cached Node, or create a temporary one without notification channels.
///
/// Used by `receive_push_notification` which runs in an Android background service
/// and only needs DB access + mailbox sync. If the app is already running, this reuses
/// the existing Node (avoiding duplicate SQLite connections). If not, it creates a
/// temporary Node that is NOT cached — so when the app later starts via
/// `build_and_cache_node`, it will create a proper Node with channels.
pub async fn get_or_build_node(data_path: PathBuf) -> anyhow::Result<Node> {
    // Fast path: return cached node if the app is already running
    {
        let guard = NODES.lock().unwrap();
        if let Some(map) = guard.as_ref() {
            if let Some(node) = map.get(&data_path) {
                return Ok(node.clone());
            }
        }
    }

    // App is not running — create a temporary node without channels (not cached)
    build_node(data_path, None, None).await
}
