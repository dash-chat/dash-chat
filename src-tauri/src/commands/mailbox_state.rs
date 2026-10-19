use std::collections::BTreeSet;
use std::future::Future;
use std::time::Duration;

use dashchat_node::{topic::TopicId, DeviceId, Node};
use futures::{future, stream, FutureExt, Stream, StreamExt};
use mailbox_client::{manager::MailboxConnectionState, sync_tracker::MailboxSyncState, MailboxId};
use tauri_plugin_subscriptions::subscription;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use crate::node::node_slot::node_stream;

/// Wait until `id` is tracked on `node` (right after a resume it may not be
/// re-registered yet), then return its watch. Errors if the node's mailbox set
/// closes (node gone).
async fn tracked_connection_state(
    node: Node,
    id: MailboxId,
) -> anyhow::Result<watch::Receiver<MailboxConnectionState>> {
    let mut active = node.mailboxes.active_mailbox_ids();
    loop {
        if let Some(mailbox) = node.mailboxes.tracked_mailbox(&id).await {
            return Ok(mailbox.connection_state());
        }
        // Re-check when the mailbox set changes, with a poll fallback in case
        // tracking changes without an active-set change.
        tokio::select! {
            r = active.changed() => r?,
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

/// Wait until a sync-state watch is available for `id`, then return it.
async fn tracked_sync_state(
    node: Node,
    id: MailboxId,
) -> anyhow::Result<watch::Receiver<MailboxSyncState<TopicId, DeviceId>>> {
    let mut active = node.mailboxes.active_mailbox_ids();
    loop {
        if let Ok(rx) = node.mailboxes.sync_tracker().sync_state(&id).await {
            return Ok(rx);
        }
        tokio::select! {
            r = active.changed() => r?,
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

/// The watch `tracked` resolves to, as a stream; empty if the node goes first.
fn watch_once_tracked<T>(
    tracked: impl Future<Output = anyhow::Result<watch::Receiver<T>>>,
) -> impl Stream<Item = T>
where
    T: Clone + Send + Sync + 'static,
{
    tracked
        .map(|rx| match rx {
            Ok(rx) => WatchStream::new(rx).left_stream(),
            Err(_) => stream::empty().right_stream(),
        })
        .flatten_stream()
}

#[subscription]
pub fn mailbox_subscribe_active_ids() -> impl Stream<Item = BTreeSet<MailboxId>> {
    node_stream(|node| WatchStream::new(node.mailboxes.active_mailbox_ids()))
        .filter_map(future::ready)
}

#[subscription]
pub fn mailbox_subscribe_cloud_id() -> impl Stream<Item = Option<MailboxId>> {
    // Recomputed on each active-set change: the cloud id is derived from the
    // active set *and* the tracked mailboxes' urls.
    node_stream(|node| {
        WatchStream::new(node.mailboxes.active_mailbox_ids()).then(move |_| {
            let node = node.clone();
            async move { crate::mailbox::cloud_mailbox_id(&node).await }
        })
    })
    .filter_map(future::ready)
}

#[subscription]
pub fn mailbox_subscribe_all_ids() -> impl Stream<Item = BTreeSet<MailboxId>> {
    node_stream(|node| WatchStream::new(node.mailboxes.sync_tracker().all_mailbox_ids()))
        .filter_map(future::ready)
}

#[subscription]
pub fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxConnectionState> {
    node_stream(move |node| watch_once_tracked(tracked_connection_state(node, mailbox_id.clone())))
        .filter_map(future::ready)
}

#[subscription]
pub fn mailbox_subscribe_sync_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxSyncState<TopicId, DeviceId>> {
    node_stream(move |node| watch_once_tracked(tracked_sync_state(node, mailbox_id.clone())))
        .filter_map(future::ready)
}
