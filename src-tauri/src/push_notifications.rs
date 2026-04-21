use dashchat_node::Node;
use push_notifications_server::client::PushNotificationsClient;
use push_notifications_server::types::{PublicKey, TopicId};
use tauri::{AppHandle, Manager};

#[cfg(mobile)]
pub mod mobile;

const PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL: &str =
    "https://push-notifications-server.production.dash-chat.dash-chat.garnix.me";

/// Returns the push notifications server URL to use.
///
/// Resolution order:
/// 1. `PUSH_NOTIFICATIONS_URL` runtime env var (E2E tests)
/// 2. `PUSH_NOTIFICATIONS_URL` compile-time env var (dev builds via mprocs / start-dev.sh)
/// 3. Production URL
pub fn push_notifications_url() -> String {
    if let Ok(url) = std::env::var("PUSH_NOTIFICATIONS_URL") {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            log::error!(
                "PUSH_NOTIFICATIONS_URL env var is not a valid URL: {url}, falling back to next option"
            );
        } else {
            return url;
        }
    }
    if let Some(url) = option_env!("PUSH_NOTIFICATIONS_URL") {
        log::info!("Using compile-time PUSH_NOTIFICATIONS_URL: {url}");
        return url.to_string();
    }
    if tauri::is_dev() {
        panic!(
            "PUSH_NOTIFICATIONS_URL must be set in dev builds (via env var or compile-time env)"
        );
    }
    PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL.to_string()
}

/// Sync all subscribed topics with the push notifications server.
///
/// Called at startup to ensure the server has the full, up-to-date list of
/// topics this device is subscribed to (replacing any stale state).
pub async fn sync_subscriptions(app_handle: &AppHandle) {
    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let topic_ids: Vec<TopicId> = match node.subscribed_topics() {
        Ok(topics) => topics
            .into_iter()
            .map(|t| TopicId::from(hex::encode(&*t)))
            .collect(),
        Err(err) => {
            log::error!("Failed to get subscribed topics: {err:?}");
            return;
        }
    };

    log::info!(
        "Syncing {} topic subscriptions with push notifications server.",
        topic_ids.len()
    );

    let client = PushNotificationsClient::new(push_notifications_url());

    if let Err(err) = client.set_subscriptions(public_key, topic_ids).await {
        log::error!("Failed to set subscriptions: {err:?}");
    }
}

/// Subscribe the current device to push notifications for the given topics.
pub async fn subscribe_to_topics(app_handle: &AppHandle, topic_ids: Vec<TopicId>) {
    if topic_ids.is_empty() {
        return;
    }

    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = PushNotificationsClient::new(push_notifications_url());

    log::info!(
        "Subscribing to {} topics on push notifications server.",
        topic_ids.len()
    );

    if let Err(err) = client.subscribe(public_key, topic_ids).await {
        log::error!("Failed to subscribe to topics: {err:?}");
    }
}

/// Listens for new topic subscriptions and registers them with the push notifications server.
pub fn spawn_topic_subscription_loop(
    app_handle: AppHandle,
    mut topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(topic_id) = topic_subscribed_rx.recv().await {
            let hex_topic = TopicId::from(hex::encode(&*topic_id));
            subscribe_to_topics(&app_handle, vec![hex_topic]).await;
        }
    });
}
