use std::collections::HashSet;

use dashchat_node::{AsBody, Node, Notification, Payload, Topic};
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{FcmToken, PublicKey, PushNotification, TopicId};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

#[cfg(target_os = "android")]
mod android;

const PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL: &str =
    "https://push-notifications-server.production.dash-chat.dash-chat.garnix.me";

/// Returns the push notifications server URL to use.
///
/// Resolution order:
/// 1. `PUSH_NOTIFICATIONS_SERVER_URL` runtime env var (E2E tests)
/// 2. `PUSH_NOTIFICATIONS_SERVER_URL` compile-time env var (set by build.rs in debug builds)
/// 3. Production URL
fn push_notifications_url() -> String {
    if let Ok(url) = std::env::var("PUSH_NOTIFICATIONS_SERVER_URL") {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            log::error!(
                "PUSH_NOTIFICATIONS_SERVER_URL env var is not a valid URL: {url}, falling back to next option"
            );
        } else {
            return url;
        }
    }
    if let Some(url) = option_env!("PUSH_NOTIFICATIONS_SERVER_URL") {
        log::info!("Using compile-time PUSH_NOTIFICATIONS_SERVER_URL: {url}");
        return url.to_string();
    }
    PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL.to_string()
}

pub fn setup_push_notifications(
    handle: AppHandle,
    topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
) {
    let h = handle.clone();

    // Re-register every time the app starts
    // This makes it so that a loss of data in the push notifications server will be recovered from
    if let Ok(PermissionState::Granted) = h.notification().permission_state() {
        match h.notification().register_for_push_notifications() {
            Ok(token) => {
                let h = h.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = register_fcm_token(h, token.clone()).await {
                        log::error!("Error registering FCM token: {:?}", err);
                    } else {
                        log::info!("Successfully registered FCM token.");
                    }
                });
            }
            Err(err) => {
                log::error!("Error registering for push notifications: {:?}.", err);
            }
        }
    }

    // React to whenever the token changes
    handle.listen("notification://new-fcm-token", move |event| {
        if let Ok(token) = serde_json::from_str::<String>(event.payload()) {
            log::warn!(
                "New FCM token: {:?}. Registering it with the push notifications server.",
                token
            );
            let h = h.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = register_fcm_token(h, token.clone()).await {
                    log::error!("Error registering FCM token: {:?}", err);
                } else {
                    log::info!("Successfully registered FCM token.");
                }
            });
        }
    });

    // Sync all subscribed topics at startup, then listen for new ones
    let h = handle.clone();
    tauri::async_runtime::spawn(async move {
        sync_subscriptions(&h).await;
    });
    spawn_topic_subscription_loop(handle, topic_subscribed_rx);
}

async fn register_fcm_token(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = PushNotificationsClient::new(push_notifications_url());

    loop {
        match client
            .register_fcm_token(public_key.clone(), FcmToken::from(token.clone()))
            .await
        {
            Ok(()) => return Ok(()),
            Err(err) => {
                log::warn!("register_fcm_token failed: {err:?}. Retrying in 1000ms.");
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    }
}

/// Sync all subscribed topics with the push notifications server.
///
/// Called at startup to ensure the server has the full, up-to-date list of
/// topics this device is subscribed to (replacing any stale state).
async fn sync_subscriptions(app_handle: &AppHandle) {
    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let topic_ids: HashSet<TopicId> = match node.subscribed_topics() {
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
async fn subscribe_to_topics(app_handle: &AppHandle, topic_ids: HashSet<TopicId>) {
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
fn spawn_topic_subscription_loop(
    app_handle: AppHandle,
    mut topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(topic_id) = topic_subscribed_rx.recv().await {
            let hex_topic = TopicId::from(hex::encode(&*topic_id));
            subscribe_to_topics(&app_handle, [hex_topic].into()).await;
        }
    });
}
