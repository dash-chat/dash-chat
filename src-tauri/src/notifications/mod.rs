mod notified_operations_store;

#[cfg(mobile)]
pub mod push_notifications;

pub(crate) use notified_operations_store::NotifiedOperationsStore;

use anyhow::Context;
use dashchat_node::{
    ChatId, DeviceId, FakeAgentId, MediaBundle, MediaMetadata, Node, Payload, Topic, TopicId,
};
use p2panda::operation::Header;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationData, NotificationExt, PermissionState};

use crate::node::AppNodeManager;

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
    notification: &dashchat_node::OpNotification,
) {
    if !are_notifications_enabled(app_handle) {
        return;
    }

    let Some(app_node_manager) = app_handle.try_state::<AppNodeManager>() else {
        return;
    };

    let Ok(node) = app_node_manager.get().await else {
        return;
    };
    let data = build_notification_data(
        &node,
        notification.topic,
        &notification.header,
        notification.payload.as_ref(),
    )
    .await;

    let Some(data) = data else { return };

    match app_node_manager
        .notified_operations_store()
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
    if let Some(bytes) = data.large_icon_bytes {
        builder = builder.large_icon_bytes(bytes);
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
/// foreground sync loop (`notification_loop` in `app_node_manager.rs`). Returns `None`
/// when the op should not produce a user-facing notification (own message, payload
/// variant we don't surface, etc.).
pub async fn build_notification_data(
    node: &Node,
    topic: TopicId,
    header: &Header,
    payload: Option<&Payload>,
) -> Option<NotificationData> {
    let sender_device_id = DeviceId::from(header.verifying_key);
    if sender_device_id == node.device_id() {
        return None;
    }

    // A blocked contact's operations are thrown away as they arrive
    // (`enforce_blocklist`), so nothing they write may reach the shade
    // either — a notification for a message the chat will never show is the
    // one way a block leaks.
    match node.projection.is_author_blocked(&sender_device_id).await {
        Ok(false) => {}
        Ok(true) => return None,
        Err(err) => {
            log::error!("Failed to check whether {sender_device_id:?} is blocked: {err:?}");
            return None;
        }
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
            return auth_control_op_notification(node, header, topic, sender_device_id, id).await;
        }
        #[cfg(not(mobile))]
        {
            let _ = (header, sender_device_id, id);
            return None;
        }
    };

    match payload {
        Payload::Chat(dashchat_node::ChatPayload::Message(content)) => {
            chat_message_notification(node, topic, sender_device_id, content, id).await
        }
        Payload::Inbox(dashchat_node::InboxPayload::ContactRequest { profile, .. }) => {
            // Entering someone's link publishes into the inbox their QR code
            // advertises, which subscribes us to it — so the requests other
            // people send them arrive on our device too. Only the inbox's
            // owner announces what is in it.
            match node.my_inbox_topics().await {
                Ok(mine) if mine.contains(&topic) => {}
                Ok(_) => return None,
                Err(err) => {
                    log::error!("Failed to load our inbox topics: {err:?}");
                    return None;
                }
            }
            // Someone we already scanned, adding us back, is completing the
            // exchange we started rather than asking for one: announcing it as
            // a new request tells the user a stranger wrote to them. Matched on
            // the device, since an exchange we started knows no agent id until
            // their ack arrives.
            match node.has_outgoing_pending_request(sender_device_id).await {
                Ok(true) => return None,
                Ok(false) => {}
                Err(err) => {
                    log::error!("Failed to load our outgoing contact requests: {err:?}");
                    return None;
                }
            }
            let chat_topic =
                Topic::direct_chat([node.fake_agent_id(), FakeAgentId::from(sender_device_id)]);
            Some(NotificationData {
                id,
                title: Some(sonix_i18n::t!("newContactRequest")),
                body: Some(profile.name.clone()),
                icon: Some("ic_stat_icon".to_string()),
                group: Some(topic.to_hex()),
                route: Some(format!("/direct-chats/{}", chat_topic.to_hex())),
                ..Default::default()
            })
        }
        // An auth/control op keeps its data in the header rather than a body:
        // the push path decodes nothing and takes the branch above, while the
        // sync path is handed what the node decoded. Same op, same answer.
        #[cfg(mobile)]
        Payload::GroupControl(_) => {
            auth_control_op_notification(node, header, topic, sender_device_id, id).await
        }
        _ => None,
    }
}

async fn chat_message_notification(
    node: &Node,
    topic: TopicId,
    sender_device_id: DeviceId,
    content: &dashchat_node::ChatMessageContent,
    id: i32,
) -> Option<NotificationData> {
    let sender_agent_id = match node.lookup_contact(sender_device_id).await {
        Ok(agent_id) => agent_id,
        Err(err) => {
            log::error!("Failed to lookup contact for sender {sender_device_id:?}: {err:?}");
            None
        }
    };

    let is_direct_chat =
        *Topic::direct_chat([node.fake_agent_id(), FakeAgentId::from(sender_device_id)]) == topic;
    if is_direct_chat {
        let accepted = match node.accepted_contact_agent_ids().await {
            Ok(accepted) => accepted,
            Err(err) => {
                log::error!("Failed to load accepted contacts: {err:?}");
                return None;
            }
        };
        if !sender_agent_id.is_some_and(|agent_id| accepted.contains(&agent_id)) {
            return None;
        }
    } else if !is_member_of(node, topic).await {
        return None;
    }

    let sender_profile = if let Some(agent_id) = sender_agent_id {
        node.projection.get_profile(agent_id).await.ok().flatten()
    } else {
        None
    };

    let sender_name = sender_profile.as_ref().map(|p| p.name.clone());
    let sender_avatar = sender_profile
        .and_then(|p| p.avatar)
        .filter(|s| s.starts_with("data:image/"));

    let chat_route = if is_direct_chat {
        format!("/direct-chats/{}", topic)
    } else {
        format!("/group-chat/{}", topic)
    };

    let message_text: &str = content.message();
    let body_text = if message_text.is_empty() {
        match content.media() {
            Some(media) => media_placeholder(media),
            None => String::new(),
        }
    } else {
        match message_text.char_indices().nth(200) {
            Some((idx, _)) => format!("{}...", &message_text[..idx]),
            None => message_text.to_string(),
        }
    };

    #[cfg_attr(not(target_os = "android"), allow(unused_mut))]
    let mut data = NotificationData {
        id,
        title: Some(sender_name.unwrap_or_else(|| sonix_i18n::t!("newMessage"))),
        body: Some(body_text),
        icon: Some("ic_stat_icon".to_string()),
        large_icon_bytes: sender_avatar,
        group: Some(topic.to_hex()),
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
        let conversation_title = if is_direct_chat {
            None
        } else {
            Some(group_title(node, topic).await)
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
        match stable_notification_id(topic.as_bytes()) {
            Ok(id) => data.id = id,
            Err(err) => log::error!(
                "Failed to derive Android MessagingStyle id from topic, falling back to random: {err:?}"
            ),
        }
        data.group = Some("dashchat.chats".to_string());
    }

    Some(data)
}

/// Whether we are still in the group `topic` names. Leaving one does not stop
/// its messages arriving — the node keeps syncing the group and rejects what
/// comes after the departure — so what is not shown in the chat must not be
/// announced either.
async fn is_member_of(node: &Node, topic: TopicId) -> bool {
    let chat_id = match ChatId::from_topic_id(topic) {
        Ok(chat_id) => chat_id,
        Err(err) => {
            log::error!("Failed to read a chat id off topic {topic}: {err:?}");
            return false;
        }
    };
    match node.get_group_members(chat_id).await {
        Ok(members) => members
            .iter()
            .any(|(member, _)| *member == node.device_id()),
        Err(err) => {
            log::error!("Failed to load the members of {topic}: {err:?}");
            false
        }
    }
}

/// Signal-style placeholder body for a media message with no caption,
/// e.g. "📷 Photo", "📎 report.pdf", "🎤 Voice message".
fn media_placeholder(media: &MediaBundle) -> String {
    if let Some(MediaMetadata::File { name, .. }) = media
        .iter()
        .find(|item| matches!(item, MediaMetadata::File { .. }))
    {
        return format!("📎 {name}");
    }
    if media
        .iter()
        .any(|item| matches!(item, MediaMetadata::VoiceNote { .. }))
    {
        return format!("🎤 {}", sonix_i18n::t!("voiceMessage"));
    }
    let photos = media
        .iter()
        .filter(|item| matches!(item, MediaMetadata::Photo { .. }))
        .count();
    match photos {
        0 => String::new(),
        1 => format!("📷 {}", sonix_i18n::t!("photo")),
        n => format!("📷 {}", sonix_i18n::t!("photosCount", { "count": n })),
    }
}

/// Resolves the latest group name for `topic_id`, falling back to a localized
/// "New group" placeholder when there's no `GroupInfo` op yet or the name is
/// empty. Used in chat-message notifications (as the MessagingStyle conversation
/// title) and in auth-control notifications (as the title for group
/// invites/adds). Mobile-only: desktop notifications use neither MessagingStyle
/// nor the auth-control variant.
#[cfg(mobile)]
async fn group_title(node: &Node, topic_id: TopicId) -> String {
    match node.get_group_info(topic_id).await {
        Ok(Some(info)) if !info.name.is_empty() => info.name,
        Ok(_) => sonix_i18n::t!("newGroup"),
        Err(err) => {
            log::error!("Failed to look up group info for notification: {err:?}");
            sonix_i18n::t!("newGroup")
        }
    }
}

/// Build the user-facing notification for a p2panda auth/control op
/// (GroupControl: Create/Add/Remove/Promote/Demote), whose data lives in the
/// header's auth extension.
///
/// Only being added to a group is announced. Signal draws the line in the
/// same place: every other membership change — someone else joining or
/// leaving, being removed oneself, an admin change — is written into the
/// conversation and never onto the shade, and the acceptance of a request is
/// not an event its sender hears about at all.
///
/// Mobile-only: on desktop we don't surface these as system notifications —
/// the chat-list row already reflects the auth event reactively.
#[cfg(mobile)]
async fn auth_control_op_notification(
    node: &Node,
    header: &Header,
    topic: TopicId,
    sender_device_id: DeviceId,
    id: i32,
) -> Option<NotificationData> {
    type GroupAction = p2panda_auth::group::GroupAction<p2panda_core::VerifyingKey>;

    let action = &header.extensions.groups_args.as_ref()?.action;

    let target_is_me = |member: &p2panda_auth::group::GroupMember<p2panda_core::VerifyingKey>| {
        matches!(
            member,
            p2panda_auth::group::GroupMember::Individual(pk) if DeviceId::from(*pk) == node.device_id()
        )
    };

    let added_me = match action {
        // A Create is either the acceptor authoring a new direct-chat space,
        // which is not a group at all, or someone creating a group with us in
        // it. The deterministic direct-chat topic with the sender tells which.
        GroupAction::Create { initial_members } => {
            *Topic::direct_chat([node.fake_agent_id(), FakeAgentId::from(sender_device_id)])
                != topic
                && initial_members.iter().any(|(m, _)| target_is_me(m))
        }
        GroupAction::Add { member, .. } => target_is_me(&member),
        GroupAction::Remove { .. } | GroupAction::Promote { .. } | GroupAction::Demote { .. } => {
            false
        }
    };
    if !added_me {
        return None;
    }

    let sender_name = match node.lookup_contact(sender_device_id).await.ok().flatten() {
        Some(agent_id) => node
            .projection
            .get_profile(agent_id)
            .await
            .ok()
            .flatten()
            .map(|profile| profile.name),
        None => None,
    };

    Some(NotificationData {
        id,
        title: Some(group_title(node, topic).await),
        body: Some(match &sender_name {
            Some(name) => sonix_i18n::t!("someoneAddedYouToTheGroup", { "name": name }),
            None => sonix_i18n::t!("someoneAddedYouToTheGroupNoName"),
        }),
        icon: Some("ic_stat_icon".to_string()),
        route: Some(format!("/group-chat/{}", topic.to_hex())),
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
