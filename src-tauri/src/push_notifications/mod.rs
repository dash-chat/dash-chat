use std::collections::HashSet;
use std::sync::Arc;

use dashchat_node::{AsBody, Node, Notification, Payload, Topic};
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{
    FcmToken, PublicKey, PushNotification, TopicId as PushTopicId,
};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

mod node_cache;
mod notification_navigation;

pub use notification_navigation::{handle_launching_notification, listen_for_notification_taps};

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
) -> anyhow::Result<()> {
    // Clear any temporary nodes that were created by push notifications before
    // the app fully started. The authoritative Node is now managed by Tauri.
    tauri::async_runtime::spawn(node_cache::clear());

    handle.manage(PushNotificationsClient::new(push_notifications_url())?);

    let h = handle.clone();

    // Re-register every time the app starts
    // This makes it so that a loss of data in the push notifications server will be recovered from
    if let Ok(PermissionState::Granted) = h.notification().permission_state() {
        match h.notification().register_for_push_notifications() {
            Ok(token) => {
                let h = h.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = register_fcm_token_with_retries(h, token.clone()).await {
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
                if let Err(err) = register_fcm_token_with_retries(h, token.clone()).await {
                    log::error!("Error registering FCM token: {:?}", err);
                } else {
                    log::info!("Successfully registered FCM token.");
                }
            });
        }
    });

    // Watcher that retries sync_subscriptions when notified of a failure.
    // Uses exponential backoff until the server is reachable, then goes
    // back to sleep until the next failure notification.
    let sync_notify = Arc::new(tokio::sync::Notify::new());

    // Sync all subscribed topics at startup
    let h = handle.clone();
    let notify = sync_notify.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = sync_subscriptions(&h).await {
            log::error!("Failed to sync subscriptions at startup: {err:?}");
            notify.notify_one();
        }
    });

    // Background watcher: retries full sync on failure with exponential backoff
    let h = handle.clone();
    spawn_subscription_sync_watcher(h, sync_notify.clone());

    // Listen for new topic subscriptions and register them with the server
    spawn_topic_subscription_loop(handle, topic_subscribed_rx, sync_notify);

    Ok(())
}

async fn register_fcm_token_with_retries(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = handle.state::<PushNotificationsClient>();

    dashchat_utils::retry_with_backoff(
        None,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(60),
        "register_fcm_token",
        || client.register_fcm_token(public_key.clone(), FcmToken::from(token.clone())),
    )
    .await
}

/// Sync all subscribed topics with the push notifications server.
///
/// Called at startup to ensure the server has the full, up-to-date list of
/// topics this device is subscribed to (replacing any stale state).
async fn sync_subscriptions(app_handle: &AppHandle) -> anyhow::Result<()> {
    let node = app_handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let topic_ids: HashSet<PushTopicId> = node
        .subscribed_topics()?
        .into_iter()
        .map(|t| PushTopicId::from(hex::encode(&*t)))
        .collect();

    log::info!(
        "Syncing {} topic subscriptions with push notifications server.",
        topic_ids.len()
    );

    let client = app_handle.state::<PushNotificationsClient>();
    client
        .update_topic_subscriptions(public_key, topic_ids)
        .await?;

    Ok(())
}

/// Subscribe the current device to push notifications for the given topics.
async fn subscribe_to_topics(
    app_handle: &AppHandle,
    topic_ids: HashSet<PushTopicId>,
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

    client
        .add_topic_subscriptions(public_key, topic_ids)
        .await?;

    Ok(())
}

/// Background watcher that retries a full subscription sync when notified.
///
/// When any subscription operation fails (likely due to no connectivity),
/// the caller notifies this watcher via `sync_notify`. The watcher then
/// retries `sync_subscriptions` with exponential backoff until it succeeds.
/// Since `sync_subscriptions` does a full replace of all topics, it covers
/// both the initial sync and any topics that failed to subscribe individually.
fn spawn_subscription_sync_watcher(app_handle: AppHandle, sync_notify: Arc<tokio::sync::Notify>) {
    tauri::async_runtime::spawn(async move {
        loop {
            sync_notify.notified().await;

            log::info!("Subscription sync watcher triggered, will retry with backoff.");

            if let Ok(_) = dashchat_utils::retry_with_backoff::<(), anyhow::Error, _, _>(
                None,
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(60),
                "sync_subscriptions",
                || sync_subscriptions(&app_handle),
            )
            .await
            {
                log::info!("Successfully synced subscriptions after retry.");
            }
        }
    });
}

/// Listens for new topic subscriptions and registers them with the push notifications server.
/// On failure, notifies the sync watcher to retry a full sync when connectivity is restored.
fn spawn_topic_subscription_loop(
    app_handle: AppHandle,
    mut topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
    sync_notify: Arc<tokio::sync::Notify>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(topic_id) = topic_subscribed_rx.recv().await {
            let hex_topic = PushTopicId::from(hex::encode(&*topic_id));
            if let Err(err) = subscribe_to_topics(&app_handle, [hex_topic].into()).await {
                log::error!("Failed to subscribe to topic: {err:?}");
                sync_notify.notify_one();
            }
        }
    });
}
