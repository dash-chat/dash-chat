//! Being added to a group reaches us as a JoinGroup in the direct chat with
//! whoever added us: of the two topics, that is the only one we are subscribed
//! to. The announcement is built from the operation on the group's own topic
//! that the JoinGroup names, which the node fetches once it has taken the
//! JoinGroup in.

use dashchat_node::{ChatId, Node};
use p2panda::operation::{Header, LogId, Operation};
use p2panda_core::{Hash, VerifyingKey};
use push_notifications_client::types::TopicId as PushTopicId;
use tauri_plugin_notification::NotificationData;

use super::receive_push_notification::{OP_POLL_INTERVAL, RECONNECT_INTERVAL};
use crate::node::node_slot;
use crate::notifications;

/// The notification for `join_group`, and the operation to record it against.
pub async fn wait_for_added_us_notification(
    node: &Node,
    join_group: &Header,
    chat_id: ChatId,
    add_member_operation_hash: Hash,
    deadline: tokio::time::Instant,
) -> Option<(NotificationData, Hash)> {
    let Some(operation) = wait_for_operation(node, add_member_operation_hash, deadline).await
    else {
        // A JoinGroup is about its receiver by construction, so there is
        // something to tell them even without the operation. Recorded against
        // the JoinGroup itself: the add operation keeps its slot, so the app
        // still announces it properly once it syncs it.
        log::warn!(
            "The operation adding us to {} did not arrive in time; announcing a generic message",
            chat_id.to_hex()
        );
        return Some((
            notifications::new_message_generic_notification(),
            join_group.hash(),
        ));
    };
    if !is_on_group_by_inviter(&operation, chat_id, join_group.verifying_key) {
        log::warn!(
            "The invitation to {} names an operation that is not its inviter's on that group",
            chat_id.to_hex()
        );
        return None;
    }
    if !wait_for_group_name(node, chat_id, deadline).await {
        log::info!(
            "The name of group {} did not arrive in time; announcing it unnamed",
            chat_id.to_hex()
        );
    }
    // Announced from the header's group args, as the push path does for any
    // group control operation; its body adds nothing to that.
    notifications::build_notification_data(node, *chat_id, &operation.header, None)
        .await
        .map(|data| (data, add_member_operation_hash))
}

async fn wait_for_operation(
    node: &Node,
    hash: Hash,
    deadline: tokio::time::Instant,
) -> Option<Operation> {
    let mut next_probe = tokio::time::Instant::now();
    loop {
        if let Ok(Some(operation)) = node.op_store.get_operation(&hash).await {
            return Some(operation);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        // A mailbox that is not active ignores the request to fetch a newly
        // joined topic, so the group's operations only arrive with a poll.
        if tokio::time::Instant::now() >= next_probe {
            crate::mailbox::probe_cloud_mailbox(node).await;
            next_probe = tokio::time::Instant::now() + RECONNECT_INTERVAL;
        }
        tokio::time::sleep(OP_POLL_INTERVAL).await;
    }
}

/// The creator names the group right after creating it, so the name is usually
/// a moment behind the invitation; announced without it, the group is "New
/// group".
async fn wait_for_group_name(node: &Node, chat_id: ChatId, deadline: tokio::time::Instant) -> bool {
    let mut next_probe = tokio::time::Instant::now();
    loop {
        if let Ok(Some(info)) = node.get_group_info(*chat_id).await {
            if !info.name.is_empty() {
                return true;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        if tokio::time::Instant::now() >= next_probe {
            crate::mailbox::probe_cloud_mailbox(node).await;
            next_probe = tokio::time::Instant::now() + RECONNECT_INTERVAL;
        }
        tokio::time::sleep(OP_POLL_INTERVAL).await;
    }
}

/// Whether `operation` was published by `inviter` on `chat_id`'s log, as the
/// operation a JoinGroup names must be.
fn is_on_group_by_inviter(operation: &Operation, chat_id: ChatId, inviter: VerifyingKey) -> bool {
    operation.header.verifying_key == inviter
        && operation.header.extensions.log_id() == LogId::from_topic(*chat_id)
}

/// Nothing runs once the push handler returns, so the group's messages only
/// wake this device if its topic reaches the push server before then.
pub async fn wait_until_group_is_subscribed_on_push_server(
    chat_id: ChatId,
    deadline: tokio::time::Instant,
) {
    let Some(app_node) = node_slot::current_node().await else {
        return;
    };
    let group_topic = PushTopicId::from(chat_id.to_hex());
    let mut topics_on_server = app_node
        .push_notifications_topic_subscriptions
        .topics_on_server
        .clone();
    let subscribed = topics_on_server.wait_for(|topics| topics.contains(&group_topic));
    if tokio::time::timeout_at(deadline, subscribed).await.is_err() {
        log::warn!(
            "Group {} was not subscribed on the push server in time",
            chat_id.to_hex()
        );
    }
}
