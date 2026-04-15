use dashchat_node::{AsBody, Node, Notification, Payload};
use jni::objects::JClass;
use jni::JNIEnv;
use p2panda_store::OperationStore;
use push_notifications_server::client::PushNotificationsClient;
use push_notifications_server::types::{FcmToken, PublicKey, PushNotification};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

#[cfg(target_os = "android")]
mod android;

pub fn setup_push_notifications(handle: AppHandle) {
    let h = handle.clone();
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
}

async fn register_fcm_token(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let node = handle.state::<Node>();
    let public_key = PublicKey::from(node.device_id().to_string());

    let client = PushNotificationsClient::new(super::push_notifications_url());

    let mut retry_count = 0;
    loop {
        match client
            .register_fcm_token(public_key.clone(), FcmToken::from(token.clone()))
            .await
        {
            Ok(()) => return Ok(()),
            Err(err) => {
                log::warn!("register_fcm_token failed: {err:?}. Retrying in 1000ms.");
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                retry_count += 1;
                if retry_count == 60 {
                    return Err(anyhow::anyhow!("Timeout. Last error: {err:?}"));
                }
            }
        }
    }
}

/// Entry point called by Android's FirebaseMessagingService when a push notification arrives.
///
/// This runs outside the normal app lifecycle — no AppHandle or managed state is available.
/// We create a temporary Node to access the local database, trigger a mailbox sync to fetch
/// the operation referenced by the push, then format a user-facing notification.
#[tauri_plugin_notification::receive_push_notification]
pub fn receive_push_notification(
    notification: NotificationData,
    context: ReceivePushNotificationContext,
) -> Option<NotificationData> {
    unsafe {
        android::setup_android_logs();
    }
    crate::i18n::init_i18n();

    log::info!("Received push notification: {notification:?}");

    // The operation hash is sent as the notification body
    let op_hash_hex = notification.body.as_deref()?;
    let op_hash: p2panda_core::Hash = op_hash_hex.parse().ok()?;

    let data_path = context.data_dir.join("studio.darksoil.dashchat");

    tauri::async_runtime::block_on(async move {
        let node = match crate::node::build_node(data_path, None).await {
            Ok(node) => node,
            Err(err) => {
                log::error!("Failed to create node for push notification: {err:?}");
                return None;
            }
        };

        // Trigger a mailbox sync to fetch the new operation
        node.mailboxes.trigger_sync();

        // Poll for the operation to arrive (up to 15 seconds)
        let mut found = false;
        for _ in 0..75 {
            match node.op_store.has_operation(op_hash).await {
                Ok(true) => {
                    found = true;
                    break;
                }
                _ => {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        if !found {
            log::warn!(
                "Operation {op_hash_hex} not found after polling, showing generic notification"
            );
            return Some(generic_notification());
        }

        // Get the operation and decode its payload
        let (header, body) = match node.op_store.get_operation(op_hash).await {
            Ok(Some(op)) => op,
            _ => {
                log::error!("Failed to get operation {op_hash_hex}");
                return Some(generic_notification());
            }
        };

        // Don't show notifications for our own messages
        let sender_device_id = dashchat_node::DeviceId::from(header.public_key);
        if sender_device_id == node.device_id() {
            return None;
        }

        let body = body?;
        let payload = match Payload::try_from_body(&body) {
            Ok(p) => p,
            Err(err) => {
                log::error!("Failed to decode payload: {err:?}");
                return Some(generic_notification());
            }
        };

        // Only show notifications for chat messages
        let dashchat_node::ChatPayload::Message(content) = (match payload {
            Payload::Chat(chat_payload) => Some(chat_payload),
            _ => None,
        })?
        else {
            return None;
        };

        // Resolve the sender's profile name via the contacts table
        let sender_name = if let Some(agent_id) = node
            .lookup_contact(sender_device_id)
            .ok()
            .flatten()
        {
            node.get_profile_for_agent(agent_id)
                .await
                .ok()
                .flatten()
                .map(|profile| profile.name)
        } else {
            None
        };

        let topic_id = header.extensions.topic;

        let title = sender_name.unwrap_or_else(|| sonix_i18n::t!("newMessage"));

        let message_text: &str = &content;
        let body_text = if message_text.len() > 200 {
            format!("{}...", &message_text[..200])
        } else {
            message_text.to_string()
        };

        Some(NotificationData {
            title: Some(title),
            body: Some(body_text),
            icon: Some("ic_stat_icon".to_string()),
            group: Some(hex::encode(&*topic_id)),
            ..Default::default()
        })
    })
}

fn generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("newMessage")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}
