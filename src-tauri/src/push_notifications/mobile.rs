use dashchat_node::{AsBody, Node, Notification, Payload, Topic};
use push_notifications_server::client::PushNotificationsClient;
use push_notifications_server::types::{FcmToken, PublicKey, PushNotification};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

#[cfg(target_os = "android")]
mod android;

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
        crate::push_notifications::sync_subscriptions(&h).await;
    });
    crate::push_notifications::spawn_topic_subscription_loop(handle, topic_subscribed_rx);
}

async fn register_fcm_token(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = PushNotificationsClient::new(super::push_notifications_url());

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
