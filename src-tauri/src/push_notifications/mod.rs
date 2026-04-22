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
    handle.manage(PushNotificationsClient::new(push_notifications_url()));

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
            // Skip if the user hasn't granted notification permission.
            // The plugin can emit cached/refreshed tokens from Firebase even
            // when the user hasn't explicitly consented in this session, so
            // we gate the server-side registration here.
            match h.notification().permission_state() {
                Ok(PermissionState::Granted) => {}
                state => {
                    log::info!(
                        "Ignoring new FCM token — notification permission is {state:?}, not Granted."
                    );
                    return;
                }
            }

            log::info!("New FCM token received. Registering it with the push notifications server.");
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
        if let Err(err) = sync_subscriptions(&h).await {
            log::error!("Failed to sync subscriptions: {err:?}");
        }
    });
    spawn_topic_subscription_loop(handle, topic_subscribed_rx);
}

async fn register_fcm_token(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = handle.state::<PushNotificationsClient>();

    let mut delay = std::time::Duration::from_secs(1);
    let max_delay = std::time::Duration::from_secs(60);
    let max_attempts: u32 = 10;

    for attempt in 1..=max_attempts {
        match client
            .register_fcm_token(public_key.clone(), FcmToken::from(token.clone()))
            .await
        {
            Ok(()) => return Ok(()),
            Err(err) => {
                if attempt == max_attempts {
                    return Err(anyhow::anyhow!(
                        "register_fcm_token failed after {max_attempts} attempts: {err:?}"
                    ));
                }
                log::warn!(
                    "register_fcm_token failed (attempt {attempt}/{max_attempts}): {err:?}. Retrying in {}s.",
                    delay.as_secs()
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(max_delay);
            }
        }
    }
    unreachable!()
}

/// Sync all subscribed topics with the push notifications server.
///
/// Called at startup to ensure the server has the full, up-to-date list of
/// topics this device is subscribed to (replacing any stale state).
async fn sync_subscriptions(app_handle: &AppHandle) -> anyhow::Result<()> {
    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let topic_ids: HashSet<TopicId> = node
        .subscribed_topics()?
        .into_iter()
        .map(|t| TopicId::from(hex::encode(&*t)))
        .collect();

    log::info!(
        "Syncing {} topic subscriptions with push notifications server.",
        topic_ids.len()
    );

    let client = app_handle.state::<PushNotificationsClient>();
    client.set_subscriptions(public_key, topic_ids).await?;

    Ok(())
}

/// Subscribe the current device to push notifications for the given topics.
async fn subscribe_to_topics(
    app_handle: &AppHandle,
    topic_ids: HashSet<TopicId>,
) -> anyhow::Result<()> {
    if topic_ids.is_empty() {
        return Ok(());
    }

    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = app_handle.state::<PushNotificationsClient>();

    log::info!(
        "Subscribing to {} topics on push notifications server.",
        topic_ids.len()
    );

    client.subscribe(public_key, topic_ids).await?;

    Ok(())
}

/// Listens for new topic subscriptions and registers them with the push notifications server.
fn spawn_topic_subscription_loop(
    app_handle: AppHandle,
    mut topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(topic_id) = topic_subscribed_rx.recv().await {
            let hex_topic = TopicId::from(hex::encode(&*topic_id));
            if let Err(err) = subscribe_to_topics(&app_handle, [hex_topic].into()).await {
                log::error!("Failed to subscribe to topic: {err:?}");
            }
        }
    });
}
