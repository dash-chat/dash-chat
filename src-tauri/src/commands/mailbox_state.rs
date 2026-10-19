use std::collections::BTreeSet;

use async_rx::StreamExt as _;
use async_stream::stream;
use dashchat_node::{topic::TopicId, DeviceId};
use futures::{Stream, StreamExt};
use mailbox_client::{manager::MailboxConnectionState, sync_tracker::MailboxSyncState, MailboxId};
use tauri_plugin_subscriptions::subscription;
use tokio_stream::wrappers::WatchStream;

use crate::node::node_slot::node_stream_values;

#[subscription]
pub fn mailbox_subscribe_active_ids() -> impl Stream<Item = BTreeSet<MailboxId>> {
    node_stream_values(|node| WatchStream::new(node.mailboxes.active_mailbox_ids()))
}

#[subscription]
pub fn mailbox_subscribe_cloud_id() -> impl Stream<Item = Option<MailboxId>> {
    // Recomputed on each active-set change: the cloud id is derived from the
    // active set *and* the tracked mailboxes' urls.
    node_stream_values(|node| {
        WatchStream::new(node.mailboxes.active_mailbox_ids()).then(move |_| {
            let node = node.clone();
            async move { crate::mailbox::cloud_mailbox_id(&node).await }
        })
    })
    .dedup()
}

#[subscription]
pub fn mailbox_subscribe_all_ids() -> impl Stream<Item = BTreeSet<MailboxId>> {
    node_stream_values(|node| WatchStream::new(node.mailboxes.sync_tracker().all_mailbox_ids()))
}

/// The frontend subscribes per active id, so a mailbox that isn't registered
/// is a race the frontend resolves itself: the active set just changed, and it
/// subscribes again from that.
#[subscription]
pub fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxConnectionState> {
    node_stream_values(move |node| {
        let id = mailbox_id.clone();
        stream! {
            // Only the watch is kept: the handle holds its sender, so keeping it
            // would hide the mailbox being unregistered.
            let watch = node.mailboxes.tracked_mailbox(&id).await.map(|mailbox| mailbox.connection_state());
            if let Some(watch) = watch {
                for await state in WatchStream::new(watch) {
                    yield state;
                }
            }
        }
    })
}

#[subscription]
pub fn mailbox_subscribe_sync_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxSyncState<TopicId, DeviceId>> {
    node_stream_values(move |node| {
        let id = mailbox_id.clone();
        stream! {
            match node.mailboxes.sync_tracker().sync_state(&id).await {
                Ok(watch) => {
                    for await state in WatchStream::new(watch) {
                        yield state;
                    }
                }
                Err(err) => log::warn!("No sync state to stream for mailbox {id}: {err:?}"),
            }
        }
    })
}
