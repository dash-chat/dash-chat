use dashchat_node::{Node, Notification};
use push_notifications_server::client::PushNotificationsClient;
use push_notifications_server::types::{PublicKey, PushNotification};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "android")]
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

pub async fn send_push_notification_to_recipients(
    app_handle: &AppHandle,
    notification: &Notification,
) {
    let node = app_handle.state::<Node>();

    // Only send push notifications for operations we authored
    let my_device_id = node.device_id();
    if dashchat_node::DeviceId::from(notification.header.public_key) != my_device_id {
        return;
    }

    let topic_id = notification.header.extensions.topic.clone();
    let authors = match node.get_authors(topic_id).await {
        Ok(authors) => authors,
        Err(err) => {
            log::error!("Failed to get authors for topic: {err:?}");
            return;
        }
    };

    let recipients: Vec<PublicKey> = authors
        .into_iter()
        .filter(|author| *author != my_device_id)
        .map(|author| PublicKey::from(author.to_string()))
        .collect();

    if recipients.is_empty() {
        return;
    }

    let client = PushNotificationsClient::new(push_notifications_url());
    let push = PushNotification {
        title: "Dash Chat".to_string(),
        body: notification.header.hash().to_hex(),
    };

    log::info!("Sending push notification to recipients: {recipients:?}.");

    if let Err(err) = client.send_push_notification(recipients, push).await {
        log::error!("Failed to send push notification: {err:?}");
    }
}
