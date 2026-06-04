mod notified_operations_store;
#[cfg(mobile)]
pub mod push_notifications;

pub(crate) use notified_operations_store::NotifiedOperationsStore;

use anyhow::Context;
use dashchat_node::{topic::TopicId, DeviceId, Header, Node, Payload, Topic};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationData, NotificationExt, PermissionState};

/// Returns `true` iff the user has both enabled notifications in app settings
/// and granted OS-level permission. On desktop the permission state is always
/// `Granted` so this collapses to the settings check.
pub(crate) fn are_notifications_enabled(handle: &AppHandle) -> bool {
    crate::settings::load_settings(handle).notifications_enabled
        && matches!(
            handle.notification().permission_state(),
            Ok(PermissionState::Granted)
        )
}

/// Show a system notification for an operation that arrived through the
/// sync pipeline (local mailbox server, p2p sync). Mirrors the push-path
/// behavior: same `NotificationData` shape, stable op-hash id,
/// foreground-suppression handled by the plugin.
pub(crate) async fn show_sync_notification(
    app_handle: &AppHandle,
    notification: &dashchat_node::Notification,
) {
    if !are_notifications_enabled(app_handle) {
        return;
    }

    let store = app_handle.state::<NotifiedOperationsStore>();
    match store
        .record_notified_operation(notification.header.hash())
        .await
    {
        Ok(false) => {
            log::debug!("Skipping sync notification: op already notified");
            return;
        }
        Ok(true) => {}
        Err(err) => {
            log::error!("Failed to record notified operation: {err:?} — proceeding anyway");
        }
    }

    let node = app_handle.state::<Node>();
    let topic_id = notification.header.extensions.topic;
    let data = build_notification_data(
        &node,
        topic_id,
        &notification.header,
        notification.payload.as_ref(),
    )
    .await;

    let Some(data) = data else { return };

    if let Err(err) = show_notification_from_data(app_handle, data) {
        log::error!("Failed to show sync-path notification: {err:?}");
    }
}

fn show_notification_from_data(handle: &AppHandle, data: NotificationData) -> anyhow::Result<()> {
    let mut builder = handle.notification().builder().id(data.id);
    if let Some(title) = data.title {
        builder = builder.title(title);
    }
    if let Some(body) = data.body {
        builder = builder.body(body);
    }
    if let Some(icon) = data.icon {
        builder = builder.icon(icon);
    }
    if let Some(group) = data.group {
        builder = builder.group(group);
    }
    if let Some(route) = data.route {
        builder = builder.route(route);
    }
    builder = builder.sound(data.sound.unwrap_or_else(|| "default".to_string()));
    if let Some(style) = data.conversation_style {
        builder = builder.conversation_style(style);
    }
    builder.show()?;
    Ok(())
}

/// Build the system notification for a freshly-processed p2panda operation.
///
/// Shared between the FCM/APNs entry point (`receive_push_notification`) and the
/// foreground sync loop (`spawn_notification_loop` in `setup.rs`). Returns `None`
/// when the op should not produce a user-facing notification (own message, payload
/// variant we don't surface, etc.).
pub async fn build_notification_data(
    node: &Node,
    topic_id: TopicId,
    header: &Header,
    payload: Option<&Payload>,
) -> Option<NotificationData> {
    let sender_device_id = DeviceId::from(header.public_key);
    if sender_device_id == node.device_id() {
        return None;
    }

    let id = match stable_notification_id(header.hash().as_bytes()) {
        Ok(id) => id,
        Err(err) => {
            log::error!("Failed to derive stable notification id: {err:?}");
            return None;
        }
    };

    let Some(payload) = payload else {
        #[cfg(mobile)]
        {
            return auth_control_op_notification(node, header, sender_device_id, id).await;
        }
        #[cfg(not(mobile))]
        {
            let _ = (header, sender_device_id, id);
            return None;
        }
    };

    match payload {
        Payload::Chat(dashchat_node::ChatPayload::Message(content)) => {
            Some(chat_message_notification(node, topic_id, sender_device_id, content, id).await)
        }
        Payload::Inbox(dashchat_node::InboxPayload::ContactRequest { code, profile }) => {
            Some(NotificationData {
                id,
                title: Some(sonix_i18n::t!("newContactRequest")),
                body: Some(profile.name.clone()),
                icon: Some("ic_stat_icon".to_string()),
                group: Some(hex::encode(&*topic_id)),
                route: Some(format!("/direct-chats/{}", code.agent_id.to_hex())),
                ..Default::default()
            })
        }
        _ => None,
    }
}

async fn chat_message_notification(
    node: &Node,
    topic_id: TopicId,
    sender_device_id: DeviceId,
    content: &dashchat_node::ChatMessageContent,
    id: i32,
) -> NotificationData {
    let sender_agent_id = match node.lookup_contact(sender_device_id).await {
        Ok(agent_id) => agent_id,
        Err(err) => {
            log::error!("Failed to lookup contact for sender {sender_device_id:?}: {err:?}");
            None
        }
    };

    let sender_name = match sender_agent_id {
        Some(agent_id) => node
            .local_store
            .get_profile(agent_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.name),
        None => None,
    };

    let direct_chat_agent_id = sender_agent_id
        .filter(|&agent_id| *Topic::direct_chat([node.agent_id(), agent_id]) == topic_id);
    let chat_route = match direct_chat_agent_id {
        Some(agent_id) => format!("/direct-chats/{}", agent_id.to_hex()),
        None => format!("/group-chat/{}", hex::encode(&*topic_id)),
    };

    let message_text: &str = content.message();
    let body_text = match message_text.char_indices().nth(200) {
        Some((idx, _)) => format!("{}...", &message_text[..idx]),
        None => message_text.to_string(),
    };

    #[cfg_attr(not(target_os = "android"), allow(unused_mut))]
    let mut data = NotificationData {
        id,
        title: Some(sender_name.unwrap_or_else(|| sonix_i18n::t!("newMessage"))),
        body: Some(body_text),
        icon: Some("ic_stat_icon".to_string()),
        group: Some(hex::encode(&*topic_id)),
        route: Some(chat_route),
        ..Default::default()
    };

    // Mobile-only: render as a chat thread (Android MessagingStyle /
    // iOS Communication Notifications). `sender_id` keeps each `Person`
    // distinct within group threads so different senders don't collapse;
    // `conversation_title` (groups only) is the group name — Android
    // surfaces it via `setConversationTitle(...) + setGroupConversation(true)`
    // and iOS via `INSendMessageIntent.speakableGroupName`.
    #[cfg(mobile)]
    {
        let conversation_title = match direct_chat_agent_id {
            Some(_) => None,
            None => Some(group_title(node, topic_id).await),
        };
        data.conversation_style = Some(tauri_plugin_notification::ConversationStyle {
            sender_id: sender_agent_id.map(|agent_id| agent_id.to_hex()),
            conversation_title,
        });
    }

    // Android-only: reuse a stable per-conversation id (so successive
    // messages update the same notification instead of stacking) and put
    // every chat thread under the "dashchat.chats" OS-level group.
    //
    // TODO: XOR the truncated topic id with a node-specific secret before
    // using it as the notification id. The first 4 bytes are
    // public-derivable, so an adversary could mine a contact whose topic
    // id shares a 4-byte LE prefix with an existing conversation and get
    // their messages collapsed into the wrong MessagingStyle thread.
    #[cfg(target_os = "android")]
    {
        match stable_notification_id(&*topic_id) {
            Ok(id) => data.id = id,
            Err(err) => log::error!(
                "Failed to derive Android MessagingStyle id from topic, falling back to random: {err:?}"
            ),
        }
        data.group = Some("dashchat.chats".to_string());
    }

    data
}

/// Resolves the latest group name for `topic_id`, falling back to a localized
/// "New group" placeholder when there's no `GroupDetails` op yet or the name is
/// empty. Used in chat-message notifications (as the MessagingStyle conversation
/// title) and in auth-control notifications (as the title for group
/// invites/adds). Mobile-only: desktop notifications use neither MessagingStyle
/// nor the auth-control variant.
#[cfg(mobile)]
async fn group_title(node: &Node, topic_id: TopicId) -> String {
    match node.get_group_details(topic_id).await {
        Ok(Some(details)) if !details.name.is_empty() => details.name,
        Ok(_) => sonix_i18n::t!("newGroup"),
        Err(err) => {
            log::error!("Failed to look up group details for notification: {err:?}");
            sonix_i18n::t!("newGroup")
        }
    }
}

/// Build the user-facing notification for a body-less p2panda auth/control op
/// (GroupControl: Create/Add/Remove/Promote/Demote). These carry their data in
/// the header's auth extension and have no body to decode. Returns `None` to
/// suppress the notification (e.g. a group action targeting someone else).
///
/// Mobile-only: on desktop we don't surface these as system notifications —
/// the chat-list row already reflects the auth event reactively.
#[cfg(mobile)]
async fn auth_control_op_notification(
    node: &Node,
    header: &Header,
    sender_device_id: DeviceId,
    id: i32,
) -> Option<NotificationData> {
    let action = &header.extensions.auth.as_ref()?.action;

    let target_is_me = |member: &p2panda_auth::group::GroupMember<p2panda_core::PublicKey>| {
        matches!(
            member,
            p2panda_auth::group::GroupMember::Individual(pk) if DeviceId::from(*pk) == node.device_id()
        )
    };

    let sender_agent_id = node.lookup_contact(sender_device_id).await.ok().flatten();
    let sender_name = match sender_agent_id {
        Some(agent_id) => node
            .local_store
            .get_profile(agent_id)
            .await
            .ok()
            .flatten()
            .map(|profile| profile.name),
        None => None,
    };

    let group_route = Some(format!(
        "/group-chat/{}",
        hex::encode(&*header.extensions.topic)
    ));

    let (title, body, route) = match action {
        // Two distinct cases share this variant: the acceptor authoring the
        // Create on a new direct-chat space (contact-request accepted), and a
        // peer creating a group with us in `initial_members` (group invite).
        // Discriminate by topic id — a direct-chat topic is deterministic from
        // the two agent ids.
        p2panda_auth::group::GroupAction::Create { initial_members } => {
            let is_direct_chat = sender_agent_id
                .map(|agent_id| {
                    *Topic::direct_chat([node.agent_id(), agent_id]) == header.extensions.topic
                })
                .unwrap_or(false);
            if is_direct_chat {
                let title = match &sender_name {
                    Some(name) => sonix_i18n::t!("contactRequestAccepted", { "name": name }),
                    None => sonix_i18n::t!("contactRequestAcceptedNoName"),
                };
                let route =
                    sender_agent_id.map(|agent_id| format!("/direct-chats/{}", agent_id.to_hex()));
                (title, None, route)
            } else {
                if !initial_members.iter().any(|(m, _)| target_is_me(m)) {
                    return None;
                }
                let body = match &sender_name {
                    Some(name) => sonix_i18n::t!("someoneAddedYouToTheGroup", { "name": name }),
                    None => sonix_i18n::t!("someoneAddedYouToTheGroupNoName"),
                };
                (
                    group_title(node, header.extensions.topic).await,
                    Some(body),
                    group_route,
                )
            }
        }
        p2panda_auth::group::GroupAction::Add { member, .. } => {
            if !target_is_me(member) {
                return None;
            }
            let body = match &sender_name {
                Some(name) => sonix_i18n::t!("someoneAddedYouToTheGroup", { "name": name }),
                None => sonix_i18n::t!("someoneAddedYouToTheGroupNoName"),
            };
            (
                group_title(node, header.extensions.topic).await,
                Some(body),
                group_route,
            )
        }
        p2panda_auth::group::GroupAction::Remove { member } => {
            if !target_is_me(member) {
                return None;
            }
            let title = match &sender_name {
                Some(name) => sonix_i18n::t!("someoneRemovedYouFromTheGroup", { "name": name }),
                None => sonix_i18n::t!("someoneRemovedYouFromTheGroupNoName"),
            };
            (title, None, group_route)
        }
        p2panda_auth::group::GroupAction::Promote { member, .. } => {
            if !target_is_me(member) {
                return None;
            }
            let title = match &sender_name {
                Some(name) => sonix_i18n::t!("someoneMadeYouAnAdmin", { "name": name }),
                None => sonix_i18n::t!("someoneMadeYouAnAdminNoName"),
            };
            (title, None, group_route)
        }
        p2panda_auth::group::GroupAction::Demote { member, .. } => {
            if !target_is_me(member) {
                return None;
            }
            let title = match &sender_name {
                Some(name) => sonix_i18n::t!("someoneRevokedAdminFromYou", { "name": name }),
                None => sonix_i18n::t!("someoneRevokedAdminFromYouNoName"),
            };
            (title, None, group_route)
        }
    };

    Some(NotificationData {
        id,
        title: Some(title),
        body,
        icon: Some("ic_stat_icon".to_string()),
        route,
        ..Default::default()
    })
}

#[cfg(mobile)]
pub fn new_message_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("youHaveANewMessage")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

#[cfg(target_os = "ios")]
pub fn synced_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("syncedWithServer")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

#[cfg(target_os = "ios")]
pub fn may_have_new_messages_generic_notification() -> NotificationData {
    NotificationData {
        title: Some(sonix_i18n::t!("mayHaveNewMessages")),
        body: None,
        icon: Some("ic_stat_icon".to_string()),
        ..Default::default()
    }
}

/// Derive a stable 32-bit notification id from a 32-byte identifier — either
/// an operation hash (per-op id; replaces on iOS / updates the same notif on
/// Android) or a topic id (Android MessagingStyle, so successive messages
/// accumulate into the same conversation thread). The first four bytes are
/// interpreted as a little-endian i32.
fn stable_notification_id(id_bytes: &[u8]) -> anyhow::Result<i32> {
    let bytes: [u8; 4] = id_bytes
        .get(..4)
        .context("id_bytes is shorter than 4 bytes")?
        .try_into()?;
    Ok(i32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_notification_id_uses_first_four_bytes_little_endian() {
        let bytes = [0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff];
        assert_eq!(
            stable_notification_id(&bytes).unwrap(),
            i32::from_le_bytes([0x01, 0x02, 0x03, 0x04])
        );
    }

    #[test]
    fn stable_notification_id_is_stable_across_calls() {
        let bytes = [0xAB; 32];
        let a = stable_notification_id(&bytes).unwrap();
        let b = stable_notification_id(&bytes).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn stable_notification_id_errors_on_short_input() {
        assert!(stable_notification_id(&[]).is_err());
        assert!(stable_notification_id(&[0x01, 0x02, 0x03]).is_err());
    }
}
