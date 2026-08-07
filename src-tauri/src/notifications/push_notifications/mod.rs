use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context;
use dashchat_utils::SingletonTaskWithRetries;
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{FcmToken, TopicId as PushTopicId, VerifyingKey};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

use crate::node::AppNodeManager;
use crate::notifications::are_notifications_enabled;

mod receive_push_notification;

#[cfg(target_os = "android")]
mod android;

const PRODUCTION_PUSH_NOTIFICATIONS_SERVER_URL: &str =
    "https://push-notifications.production.darksoil.studio";

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
        log::info!("Notifications settings were updated: synchronizing status with the push notifications server.");
        push_task.trigger();
        sync_task.trigger();
    });

    let push_task = push_notifications_registration_task.clone();
    let sync_task = sync_topic_subscriptions_task.clone();
    // React to whenever the token changes
    handle.listen("notification://new-fcm-token", move |_event| {
        log::info!("New FCM token: synchronizing status with the push notifications server.");
        push_task.trigger();
        sync_task.trigger();
    });

    // Listen for new topic subscriptions and register them with the server
    spawn_topic_subscription_loop(handle, topic_subscribed_rx, sync_topic_subscriptions_task);

    Ok(())
}

/// If notifications are currently enabled, get the FCM token and register it with the server
/// If they're not, unregister the FCM token from the server
async fn update_push_notifications_registration(handle: AppHandle) -> anyhow::Result<()> {
    let node = handle
        .try_state::<AppNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("app node not managed yet"))?
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let verifying_key = VerifyingKey::from(node.device_id().to_string());
    let client = handle.state::<PushNotificationsClient>();

    if are_notifications_enabled(&handle) {
        log::info!("Notifications are enabled: registering FCM token.");
        let token = handle
            .notification()
            .register_for_push_notifications()
            .context("register_for_push_notifications failed")?;
        client
            .register_fcm_token(verifying_key.clone(), FcmToken::from(token.clone()))
            .await
            .context("register_fcm_token failed")?;
        log::info!("Successfully registered FCM token.");
    } else {
        log::info!("Notifications are disabled: unregistering FCM token.");
        client
            .unregister_fcm_token(verifying_key.clone())
            .await
            .context("unregister_fcm_token failed")?;
        log::info!("Successfully unregistered FCM token.");
    }
    Ok(())
}

/// If notifications are enabled, sync all subscribed topics with the push notifications server.
/// If they're not, remove all topic subscriptions from it.
async fn sync_subscriptions(app_handle: AppHandle) -> anyhow::Result<()> {
    let node = app_handle
        .try_state::<AppNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("app node not managed yet"))?
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let verifying_key = VerifyingKey::from(node.device_id().to_string());

    let topic_ids = if are_notifications_enabled(&app_handle) {
        let topic_ids: HashSet<PushTopicId> = node
            .subscribed_topics()
            .await?
            .into_iter()
            .map(|t| PushTopicId::from(t.to_hex()))
            .collect();
        topic_ids
    } else {
        HashSet::new()
    };

    log::info!(
        "Syncing {} topic subscriptions with push notifications server.",
        topic_ids.len()
    );

    let client = app_handle.state::<PushNotificationsClient>();
    client
        .update_topic_subscriptions(verifying_key, topic_ids)
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

    let node = app_handle
        .try_state::<AppNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("app node not managed yet"))?
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let verifying_key = VerifyingKey::from(node.device_id().to_string());

    let client = app_handle.state::<PushNotificationsClient>();

    log::info!(
        "Subscribing to {} topics on push notifications server.",
        topic_ids.len()
    );

    client
        .add_topic_subscriptions(verifying_key, topic_ids)
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
            let hex_topic = PushTopicId::from(topic_id.to_hex());
            if let Err(err) = subscribe_to_topics(&app_handle, [hex_topic].into()).await {
                log::error!("Failed to subscribe to topic: {err:?}");
                sync_topic_subscriptions_task.trigger();
            }
        }
    });
}
