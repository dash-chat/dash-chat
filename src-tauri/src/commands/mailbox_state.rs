use std::collections::BTreeSet;

use dashchat_node::{topic::TopicId, DeviceId};
use futures::{Stream, StreamExt};
use mailbox_client::{manager::MailboxConnectionState, sync_tracker::LogSyncState, MailboxId};
use tauri_plugin_subscriptions::subscription;
use tokio_stream::wrappers::WatchStream;

use crate::node::node_slot::node_stream;

#[subscription]
pub fn mailbox_subscribe_active_ids() -> impl Stream<Item = BTreeSet<MailboxId>> {
    node_stream(|node| WatchStream::new(node.mailboxes.active_mailbox_ids()))
}

/// Recomputed on every active-set change: the cloud id is matched against the
/// tracked mailboxes' URLs.
#[subscription]
pub fn mailbox_subscribe_cloud_id() -> impl Stream<Item = Option<MailboxId>> {
    node_stream(|node| {
        WatchStream::new(node.mailboxes.active_mailbox_ids()).then(move |_| {
            let node = node.clone();
            async move { crate::mailbox::cloud_mailbox_id(&node).await }
        })
    })
}

/// The state of the mailbox once it is tracked (a rebuilt node registers its
/// mailboxes after it starts), ending when that mailbox is unregistered. The
/// frontend subscribes per active mailbox id, so it drops this subscription
/// then anyway.
#[subscription]
pub fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxConnectionState> {
    node_stream(move |node| {
        let mailboxes = node.mailboxes.clone();
        let mailbox_id = mailbox_id.clone();
        WatchStream::new(node.mailboxes.active_mailbox_ids())
            .then(move |_| {
                let mailboxes = mailboxes.clone();
                let mailbox_id = mailbox_id.clone();
                async move { mailboxes.tracked_mailbox(&mailbox_id).await }
            })
            .filter_map(|tracked| async move { tracked })
            .take(1)
            .flat_map(|tracked| WatchStream::new(tracked.connection_state()))
    })
}

#[subscription]
pub fn mailbox_subscribe_sync_state_for_log(
    topic_id: TopicId,
    author: DeviceId,
) -> impl Stream<Item = LogSyncState> {
    node_stream(move |node| {
        futures::stream::once(async move {
            node.mailboxes
                .sync_tracker()
                .sync_state_for_log(&topic_id, &author)
                .await
        })
        .filter_map(|sync_state| async move {
            sync_state
                .inspect_err(|err| log::warn!("Failed to load a log's sync state: {err:?}"))
                .ok()
        })
        .flat_map(WatchStream::new)
    })
}
