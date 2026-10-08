use std::sync::Arc;

use anyhow::Context;
use backon::{ExponentialBuilder, Retryable};
use dashchat_utils::WakeEarlyOn;
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{FcmToken, VerifyingKey};
use tauri::{AppHandle, EventId, Listener, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::{push_notifications_url, NEW_FCM_TOKEN_EVENT, NOTIFICATIONS_ENABLED_UPDATED_EVENT};
use crate::node::AppNodeManager;
use crate::notifications::{are_notifications_enabled, run_plugin_call};

/// Keeps the FCM token registered with the push notifications server while
/// notifications are enabled, and unregistered while they are not: now, so a
/// loss of data in the server is recovered from, and whenever either changes.
pub(crate) struct RegisterPushNotificationsTokenTask {
    handle: AppHandle,
    notifications_enabled_listener: EventId,
    new_token_listener: EventId,
    tracker: TaskTracker,
    token: CancellationToken,
}

impl RegisterPushNotificationsTokenTask {
    pub(crate) fn spawn(handle: AppHandle) -> anyhow::Result<Self> {
        let client = PushNotificationsClient::new(push_notifications_url())?;
        let changed = Arc::new(Notify::new());

        let notifications_enabled_changed = changed.clone();
        let notifications_enabled_listener =
            handle.listen(NOTIFICATIONS_ENABLED_UPDATED_EVENT, move |_event| {
                notifications_enabled_changed.notify_one();
            });

        let token_changed = changed.clone();
        let new_token_listener = handle.listen(NEW_FCM_TOKEN_EVENT, move |_event| {
            token_changed.notify_one();
        });

        let tracker = TaskTracker::new();
        let token = CancellationToken::new();
        tracker.spawn(
            token
                .clone()
                .run_until_cancelled_owned(keep_token_registered(handle.clone(), client, changed)),
        );
        Ok(Self {
            handle,
            notifications_enabled_listener,
            new_token_listener,
            tracker,
            token,
        })
    }

    /// Stop, and wait until stopped.
    pub(crate) async fn shutdown(&self) {
        self.handle.unlisten(self.notifications_enabled_listener);
        self.handle.unlisten(self.new_token_listener);
        self.token.cancel();
        self.tracker.close();
        self.tracker.wait().await;
    }
}

async fn keep_token_registered(
    handle: AppHandle,
    client: PushNotificationsClient,
    changed: Arc<Notify>,
) {
    let update_registration = || register_or_unregister_token(&handle, &client);
    loop {
        let result = update_registration
            .retry(ExponentialBuilder::new().with_jitter().without_max_times())
            .wake_early_on(changed.clone())
            .notify(|err, delay| {
                log::warn!("Failed to update the push notifications token registration, retrying in {delay:?}: {err:?}")
            })
            .await;
        if let Err(err) = result {
            log::warn!("Gave up updating the push notifications token registration: {err:?}");
        }
        changed.notified().await;
    }
}

async fn register_or_unregister_token(
    handle: &AppHandle,
    client: &PushNotificationsClient,
) -> anyhow::Result<()> {
    let node = handle
        .try_state::<AppNodeManager>()
        .ok_or_else(|| anyhow::anyhow!("app node not managed yet"))?
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let verifying_key = VerifyingKey::from(node.device_id().to_string());

    if are_notifications_enabled(handle) {
        log::info!("Notifications are enabled: registering FCM token.");
        let h = handle.clone();
        let token = run_plugin_call(move || h.notification().register_for_push_notifications())
            .await?
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
