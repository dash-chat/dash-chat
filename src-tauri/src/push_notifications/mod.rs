use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use dashchat_node::{AsBody, Node, Notification, Payload, Topic};
use dashchat_utils::SingletonTaskWithRetries;
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{
    FcmToken, PublicKey, PushNotification, TopicId as PushTopicId,
};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

mod node_cache;
mod notification_navigation;

pub use notification_navigation::setup_notification_navigation;

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
    let push_notifications_registration_task = SingletonTaskWithRetries::new(
        "push_notifications_registration",
        None,
        Duration::from_secs(1),
        Duration::from_secs(60),
        move || {
            let h = h.clone();
            update_push_notifications_registration(h)
        },
    );

    let h = handle.clone();
    let sync_topic_subscriptions_task = SingletonTaskWithRetries::new(
        "sync_topic_subscriptions",
        None,
        Duration::from_secs(1),
        Duration::from_secs(60),
        move || {
            let h = h.clone();
            sync_subscriptions(h)
        },
    );

    // Re-register every time the app starts
    // This makes it so that a loss of data in the push notifications server will be recovered from
    push_notifications_registration_task.trigger();

    // Sync all subscribed topics at startup
    sync_topic_subscriptions_task.trigger();

    let push_task = push_notifications_registration_task.clone();
    let sync_task = sync_topic_subscriptions_task.clone();
    handle.listen("settings://updated-notifications_enabled", move |_event| {
        push_task.trigger();
        sync_task.trigger();
    });

    let push_task = push_notifications_registration_task.clone();
    let sync_task = sync_topic_subscriptions_task.clone();
    // React to whenever the token changes
    handle.listen("notification://new-fcm-token", move |_event| {
        push_task.trigger();
        sync_task.trigger();
    });

    // Listen for new topic subscriptions and register them with the server
    spawn_topic_subscription_loop(handle, topic_subscribed_rx, sync_topic_subscriptions_task);

    Ok(())
}

fn are_notifications_enabled(handle: &AppHandle) -> bool {
    crate::settings::load_settings(handle).notifications_enabled
        && matches!(
            handle.notification().permission_state(),
            Ok(PermissionState::Granted)
        )
}

/// If notifications are currently enabled, get the FCM token and register it with the server
/// If they're not, unregister the FCM token from the server
async fn update_push_notifications_registration(handle: AppHandle) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());
    let client = handle.state::<PushNotificationsClient>();

    if are_notifications_enabled(&handle) {
        let token = handle
            .notification()
            .register_for_push_notifications()
            .context("register_for_push_notifications failed")?;
        client
            .register_fcm_token(public_key.clone(), FcmToken::from(token.clone()))
            .await
            .context("register_fcm_token failed")?;
    } else {
        client
            .unregister_fcm_token(public_key.clone())
            .await
            .context("unregister_fcm_token failed")?;
    }
    Ok(())
}

/// Sync all subscribed topics with the push notifications server.
///
/// Called at startup to ensure the server has the full, up-to-date list of
/// topics this device is subscribed to (replacing any stale state).
async fn sync_subscriptions(app_handle: AppHandle) -> anyhow::Result<()> {
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

/// Listens for new topic subscriptions and registers them with the push notifications server.
/// On failure, notifies the sync topic subscriptions task trigger a full sync when connectivity is restored.
fn spawn_topic_subscription_loop(
    app_handle: AppHandle,
    mut topic_subscribed_rx: tokio::sync::mpsc::Receiver<dashchat_node::topic::TopicId>,
    sync_topic_subscriptions_task: SingletonTaskWithRetries,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(topic_id) = topic_subscribed_rx.recv().await {
            let hex_topic = PushTopicId::from(hex::encode(&*topic_id));
            if let Err(err) = subscribe_to_topics(&app_handle, [hex_topic].into()).await {
                log::error!("Failed to subscribe to topic: {err:?}");
                sync_topic_subscriptions_task.trigger();
            }
        }
    });
}
