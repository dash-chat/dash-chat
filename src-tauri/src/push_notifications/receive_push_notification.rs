use std::path::PathBuf;

use anyhow::{anyhow, Context};
use dashchat_node::{AsBody, Payload, Topic};
use p2panda_store::LogStore;
use tauri_plugin_notification::*;

#[cfg(target_os = "android")]
use super::android::setup_android_logs;

#[cfg(target_os = "android")]
static ANDROID_LOGS_ONCE: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "ios")]
static IOS_LOGGER_ONCE: std::sync::Once = std::sync::Once::new();

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
    #[cfg(target_os = "android")]
    ANDROID_LOGS_ONCE.call_once(|| unsafe {
        setup_android_logs();
    });
    #[cfg(target_os = "ios")]
    IOS_LOGGER_ONCE.call_once(|| {
        let _ = oslog::OsLogger::new("studio.darksoil.dashchat.PushNotificationsExtension")
            .level_filter(log::LevelFilter::Debug)
            .init();
    });
    crate::i18n::init_i18n();

    log::info!("Received push notification: {notification:?}");

    let data_path = context.data_dir.join("studio.darksoil.dashchat");

    log::info!(
        "Using data path to get or build the dash chat node: {:?}.",
        data_path
    );

    tauri::async_runtime::block_on(async move {
        match handle_push_notification(notification, data_path).await {
            Ok(result) => {
                if let Some(data) = &result {
                    log::info!(
                        "Successfully processed push notification, showing notification: {:?}.",
                        data
                    );
                } else {
                    log::info!(
                        "Successfully processed push notification, no actual notification needs to be shown.",
                    );
                    // On iOS, alert notifications must be shown. If we return None here,
                    // iOS will display a notification with title = topic_id, and body = author:seq_num
                    // Show a generic notification instead
                    // TODO: apply for the exception to Apple that allows apps to not need to show a notification
                    // https://developer.apple.com/contact/request/notification-service
                    #[cfg(target_os = "ios")]
                    return Some(synced_generic_notification());
                }
                result
            }
            Err(err) => {
                log::error!("Failed to handle push notification: {err:?}");
                // On iOS, returning None here would let iOS fall back to the
                // raw APNS payload (topic_id as title, author:seq as body).
                // Show a generic fallback so the user sees something readable.
                #[cfg(target_os = "ios")]
                return Some(may_have_new_messages_generic_notification());
                #[cfg(not(target_os = "ios"))]
                None
            }
        }
    })
}

async fn handle_push_notification(
    notification: NotificationData,
    data_path: PathBuf,
) -> anyhow::Result<Option<NotificationData>> {
    // Title = topic ID (hex), Body = operation ID ("author_hex:seq_num")
    let topic_hex = notification
        .title
        .as_deref()
        .context("notification has no title")?;
    let op_id = notification
        .body
        .as_deref()
        .context("notification has no body")?;

    let (author_hex, seq_str) = op_id
        .split_once(':')
        .context("op_id missing ':' separator")?;
    let seq_num: u64 = seq_str.parse().context("failed to parse seq_num")?;

    let author_bytes: [u8; 32] = hex::decode(author_hex)
        .context("failed to hex-decode author")?
        .try_into()
        .map_err(|_| anyhow!("author bytes are not 32 bytes long"))?;
    let public_key = p2panda_core::PublicKey::from_bytes(&author_bytes)
        .context("failed to construct public key")?;

    let topic_bytes: [u8; 32] = hex::decode(topic_hex)
        .context("failed to hex-decode topic")?
        .try_into()
        .map_err(|_| anyhow!("topic bytes are not 32 bytes long"))?;
    let topic_id = dashchat_node::topic::TopicId::from(topic_bytes);

    let node = super::node_cache::get_node(data_path)
        .await
        .context("failed to get node")?;

    log::info!("dashchat node built successfully.");

    // Trigger a mailbox sync to fetch the new operation
    node.mailboxes.trigger_sync();

    // Poll for the operation to arrive (up to 15 seconds)
    // PERF: consider adding the ability for the op store to notify when an op is stored,
    //     instead of polling
    let mut found = false;
    for _ in 0..75 {
        let log = node
            .op_store
            .get_log(&public_key, &topic_id, Some(seq_num))
            .await;
        if let Ok(Some(entries)) = &log {
            if !entries.is_empty() {
                found = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    if !found {
        log::warn!(
            "Operation {op_id} in topic {topic_hex} not found after polling, showing generic notification"
        );
        return Ok(Some(new_message_generic_notification()));
    }

    // Get the operation
    let entries = node
        .op_store
        .get_log(&public_key, &topic_id, Some(seq_num))
        .await
        .map_err(|err| anyhow!("failed to read op log: {err:?}"))?
        .context("op log unexpectedly empty")?;
    let (header, body) = entries
        .into_iter()
        .next()
        .context("op log has no entries")?;

    // Don't show notifications for our own messages
    let sender_device_id = dashchat_node::DeviceId::from(header.public_key);
    if sender_device_id == node.device_id() {
        return Ok(None);
    }

    let body = body.context("operation has no body")?;
    let payload = Payload::try_from_body(&body)
        .map_err(|err| anyhow!("failed to decode payload: {err:?}"))?;

    match payload {
        Payload::Chat(dashchat_node::ChatPayload::Message(content)) => {
            // Resolve the sender's agent ID and profile name via the contacts table
            let sender_agent_id = match node.lookup_contact(sender_device_id).await {
                Ok(agent_id) => agent_id,
                Err(err) => {
                    log::error!(
                        "Failed to lookup contact for sender {sender_device_id:?}: {err:?}"
                    );
                    None
                }
            };

            let sender_name = if let Some(agent_id) = sender_agent_id {
                node.get_profile_for_agent(agent_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|profile| profile.name)
            } else {
                None
            };

            let title = sender_name.unwrap_or_else(|| sonix_i18n::t!("newMessage"));

            // Determine the chat route for notification tap navigation
            let chat_route = sender_agent_id
                .filter(|&agent_id| {
                    let direct_topic = Topic::direct_chat([node.agent_id(), agent_id]);
                    *direct_topic == topic_id
                })
                .map(|agent_id| format!("/direct-chats/{}", agent_id.to_hex()))
                .unwrap_or_else(|| format!("/group-chat/{}", hex::encode(&*topic_id)));

            // Don't show notification if the user is already viewing this chat
            if is_viewing_chat(&chat_route) {
                log::info!("Suppressing push notification: user is viewing the active chat");
                return Ok(None);
            }

            let message_text: &str = &content;
            let body_text = match message_text.char_indices().nth(200) {
                Some((idx, _)) => format!("{}...", &message_text[..idx]),
                None => message_text.to_string(),
            };

            Ok(Some(NotificationData {
                title: Some(title),
                body: Some(body_text),
                icon: Some("ic_stat_icon".to_string()),
                group: Some(hex::encode(&*topic_id)),
                ..Default::default()
            }))
        }
        Payload::Inbox(dashchat_node::InboxPayload::ContactRequest { profile, .. }) => {
            Ok(Some(NotificationData {
                title: Some(sonix_i18n::t!("newContactRequest")),
                body: Some(profile.name),
                icon: Some("ic_stat_icon".to_string()),
                group: Some(topic_hex.to_string()),
                ..Default::default()
            }))
        }
        _ => Ok(None),
    }
}

fn new_message_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("youHaveANewMessage")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

fn synced_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("syncedWithServer")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

fn may_have_new_messages_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("mayHaveNewMessages")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

/// Checks whether the main window's current URL path matches the given chat route.
fn is_viewing_chat(chat_route: &str) -> bool {
    let handle = match crate::APP_HANDLE.get() {
        Some(h) => h,
        None => return false,
    };

    use tauri::Manager;
    let window = match handle.get_webview_window("main") {
        Some(w) => w,
        None => return false,
    };

    let url = match window.url() {
        Ok(u) => u,
        Err(_) => return false,
    };

    url.path() == chat_route
}
