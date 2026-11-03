use std::collections::BTreeSet;
use std::time::Duration;

use async_rx::StreamExt as _;
use async_stream::stream;
use dashchat_node::{topic::TopicId, DeviceId};
use futures::{Stream, StreamExt};
use mailbox_client::{MailboxConnectionState, MailboxId, MailboxSyncState};
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

/// Right after a node swap the mailbox may not be registered on the new node
/// yet, so the lookup waits for the active set to change until it is.
#[subscription]
pub fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
) -> impl Stream<Item = MailboxConnectionState> {
    node_stream_values(move |node| {
        let id = mailbox_id.clone();
        stream! {
            let mut active = node.mailboxes.active_mailbox_ids();
            let watch = loop {
                if let Some(mailbox) = node.mailboxes.tracked_mailbox(&id).await {
                    break mailbox.connection_state();
                }
                if active.changed().await.is_err() {
                    return;
                }
            };
            for await state in WatchStream::new(watch) {
                yield state;
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
            let watch = loop {
                match node.mailboxes.sync_tracker().sync_state(&id).await {
                    Ok(watch) => break watch,
                    Err(err) => {
                        log::warn!("Retrying the sync state of mailbox {id}: {err:?}");
                        tokio::time::sleep(SYNC_STATE_RETRY).await;
                    }
                }
            };
            for await state in WatchStream::new(watch) {
                yield state;
            }
        }
    })
}

const SYNC_STATE_RETRY: Duration = Duration::from_millis(500);
