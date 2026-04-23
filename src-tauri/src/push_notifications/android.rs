use std::{
    ffi::CStr,
    io::BufRead,
    os::fd::{AsRawFd, FromRawFd},
};

use dashchat_node::{AsBody, Payload, Topic};
use jni::objects::JClass;
use jni::JNIEnv;
use log::Level;
use p2panda_store::LogStore;
use tauri_plugin_notification::*;

pub fn android_log(level: Level, tag: &CStr, msg: &CStr) {
    let prio = match level {
        Level::Error => ndk_sys::android_LogPriority::ANDROID_LOG_ERROR,
        Level::Warn => ndk_sys::android_LogPriority::ANDROID_LOG_WARN,
        Level::Info => ndk_sys::android_LogPriority::ANDROID_LOG_INFO,
        Level::Debug => ndk_sys::android_LogPriority::ANDROID_LOG_DEBUG,
        Level::Trace => ndk_sys::android_LogPriority::ANDROID_LOG_VERBOSE,
    };
    unsafe {
        ndk_sys::__android_log_write(prio.0 as _, tag.as_ptr(), msg.as_ptr());
    }
}

pub unsafe fn setup_android_logs() {
    let logpipe = {
        let mut logpipe: [std::os::fd::RawFd; 2] = Default::default();
        libc::pipe(logpipe.as_mut_ptr());
        libc::dup2(logpipe[1], libc::STDOUT_FILENO);
        libc::dup2(logpipe[1], libc::STDERR_FILENO);

        logpipe.map(|fd| unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) })
    };
    std::thread::spawn(move || {
        let tag = std::ffi::CStr::from_bytes_with_nul(b"RustStdoutStderr\0").unwrap();
        let file = std::fs::File::from_raw_fd(logpipe[0].as_raw_fd());
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            if let Ok(len) = reader.read_line(&mut buffer) {
                if len == 0 {
                    break;
                } else if let Ok(msg) = std::ffi::CString::new(buffer.clone()) {
                    android_log(Level::Info, tag, &msg);
                }
            }
        }
    });
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
        setup_android_logs();
    }
    crate::i18n::init_i18n();

    log::info!("Received push notification: {notification:?}");

    // Title = topic ID (hex), Body = operation ID ("author_hex:seq_num")
    let topic_hex = notification.title.as_deref()?;
    let op_id = notification.body.as_deref()?;

    // Parse "author_hex:seq_num"
    let (author_hex, seq_str) = op_id.split_once(':')?;
    let seq_num: u64 = seq_str.parse().ok()?;

    let author_bytes: [u8; 32] = hex::decode(author_hex).ok()?.try_into().ok()?;
    let public_key = p2panda_core::PublicKey::from_bytes(&author_bytes).ok()?;

    let topic_bytes: [u8; 32] = hex::decode(topic_hex).ok()?.try_into().ok()?;
    let topic_id = dashchat_node::topic::TopicId::from(topic_bytes);

    let data_path = context.data_dir.join("studio.darksoil.dashchat");

    tauri::async_runtime::block_on(async move {
        let node = match super::node_cache::get_node(data_path).await {
            Ok(node) => node,
            Err(err) => {
                log::error!("Failed to get node for push notification: {err:?}");
                return None;
            }
        };

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
            return Some(generic_notification());
        }

        // Get the operation
        let entries = node
            .op_store
            .get_log(&public_key, &topic_id, Some(seq_num))
            .await
            .ok()?;
        let (header, body) = entries?.into_iter().next()?;

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
                return None;
            }
        };

        match payload {
            Payload::Chat(dashchat_node::ChatPayload::Message(content)) => {
                // Resolve the sender's agent ID and profile name via the contacts table
                let sender_agent_id = node.lookup_contact(sender_device_id).ok().flatten();

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
                    return None;
                }

                let message_text: &str = &content;
                let body_text = match message_text.char_indices().nth(200) {
                    Some((idx, _)) => format!("{}...", &message_text[..idx]),
                    None => message_text.to_string(),
                };

                Some(NotificationData {
                    title: Some(title),
                    body: Some(body_text),
                    icon: Some("ic_stat_icon".to_string()),
                    group: Some(hex::encode(&*topic_id)),
                    ..Default::default()
                })
            }
            Payload::Inbox(dashchat_node::InboxPayload::ContactRequest { profile, .. }) => {
                Some(NotificationData {
                    title: Some(sonix_i18n::t!("newContactRequest")),
                    body: Some(profile.name),
                    icon: Some("ic_stat_icon".to_string()),
                    group: Some(topic_hex.to_string()),
                    ..Default::default()
                })
            }
            _ => None,
        }
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
