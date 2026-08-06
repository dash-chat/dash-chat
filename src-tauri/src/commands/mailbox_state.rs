use std::collections::BTreeSet;
use std::future::Future;
use std::time::Duration;

use dashchat_node::{topic::TopicId, DeviceId, Node};
use mailbox_client::{manager::MailboxConnectionState, sync_tracker::MailboxSyncState, MailboxId};
use serde::Serialize;
use tauri::{ipc::Channel, State};
use tokio::sync::watch;

use crate::node::AppNodeManager;

/// The current node, or the next one built after a swap. Returns `None` only when
/// the app is shutting down (the generation sender was dropped).
async fn wait_for_node(app_node_manager: &AppNodeManager, generation: &mut watch::Receiver<u64>) -> Option<Node> {
    loop {
        if let Ok(node) = app_node_manager.get().await {
            return Some(node);
        }
        // Paused/rebuilding: wait for the next node generation.
        if generation.changed().await.is_err() {
            return None;
        }
    }
}

/// Spawn a detached task that keeps `on_event` fed across node swaps. Whenever the
/// node is (re)built it calls `subscribe` to get that node's `watch::Receiver` and
/// forwards its changes; when the node is paused/swapped it re-binds to the next
/// one. The task ends only when the frontend drops the Channel or the app exits.
///
/// Correctness: only the `watch::Receiver` is held across the forward loop, never
/// a `Node` clone, so a paused node can still be fully torn down (0xdead10cc). The
/// `subscribe` future *may* hold a node clone transiently while it waits for a
/// resource to (re)appear, but it is raced against the generation signal, so a
/// pause aborts it and releases the clone.
fn spawn_watch_forwarder<T, F, Fut>(app_node_manager: AppNodeManager, on_event: Channel<T>, subscribe: F)
where
    T: Serialize + Clone + Send + Sync + 'static,
    F: Fn(Node) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<watch::Receiver<T>>> + Send,
{
    tokio::spawn(async move {
        let mut generation = app_node_manager.subscribe_generation();
        while let Some(node) = wait_for_node(&app_node_manager, &mut generation).await {
            // Acquire this node's watch, racing a swap so a pause mid-subscribe
            // aborts (dropping the subscribe future's transient node clone).
            let rx = tokio::select! {
                result = subscribe(node) => result.ok(),
                _ = generation.changed() => None,
            };
            let Some(mut rx) = rx else { continue };
            if on_event.send(rx.borrow_and_update().clone()).is_err() {
                return; // frontend unsubscribed
            }
            loop {
                tokio::select! {
                    changed = rx.changed() => {
                        if changed.is_err() {
                            break; // node dropped
                        }
                        if on_event.send(rx.borrow_and_update().clone()).is_err() {
                            return;
                        }
                    }
                    _ = generation.changed() => break, // node swapped
                }
            }
        }
    });
}

/// Wait until `id` is tracked on `node` (right after a resume it may not be
/// re-registered yet), then return its watch. Errors if the node's mailbox set
/// closes (node gone), so the forwarder re-binds to the next node.
async fn tracked_connection_state(
    node: &Node,
    id: &MailboxId,
) -> anyhow::Result<watch::Receiver<MailboxConnectionState>> {
    let mut active = node.mailboxes.active_mailbox_ids();
    loop {
        if let Some(mailbox) = node.mailboxes.tracked_mailbox(id).await {
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
    node: &Node,
    id: &MailboxId,
) -> anyhow::Result<watch::Receiver<MailboxSyncState<TopicId, DeviceId>>> {
    let mut active = node.mailboxes.active_mailbox_ids();
    loop {
        if let Ok(rx) = node.mailboxes.sync_tracker().sync_state(id).await {
            return Ok(rx);
        }
        tokio::select! {
            r = active.changed() => r?,
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

#[tauri::command]
pub async fn mailbox_subscribe_active_ids(
    on_event: Channel<BTreeSet<MailboxId>>,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    spawn_watch_forwarder(app_node_manager.inner().clone(), on_event, |node| async move {
        Ok(node.mailboxes.active_mailbox_ids())
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_cloud_id(
    on_event: Channel<Option<MailboxId>>,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    // Custom forwarder: the cloud id is derived from the active-id set *and* the
    // node (it inspects tracked mailboxes), so it recomputes on each active-set
    // change. `node` is held only for the current generation's forward loop and
    // dropped on swap, so it never keeps a paused node alive.
    let app_node_manager = app_node_manager.inner().clone();
    tokio::spawn(async move {
        let mut generation = app_node_manager.subscribe_generation();
        while let Some(node) = wait_for_node(&app_node_manager, &mut generation).await {
            let mut active = node.mailboxes.active_mailbox_ids();
            if on_event
                .send(crate::mailbox::cloud_mailbox_id(&node).await)
                .is_err()
            {
                return;
            }
            loop {
                tokio::select! {
                    changed = active.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        if on_event
                            .send(crate::mailbox::cloud_mailbox_id(&node).await)
                            .is_err()
                        {
                            return;
                        }
                    }
                    _ = generation.changed() => break,
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_all_ids(
    on_event: Channel<BTreeSet<MailboxId>>,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    spawn_watch_forwarder(app_node_manager.inner().clone(), on_event, |node| async move {
        Ok(node.mailboxes.sync_tracker().all_mailbox_ids())
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxConnectionState>,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    spawn_watch_forwarder(app_node_manager.inner().clone(), on_event, move |node| {
        let id = mailbox_id.clone();
        async move { tracked_connection_state(&node, &id).await }
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_sync_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxSyncState<TopicId, DeviceId>>,
    app_node_manager: State<'_, AppNodeManager>,
) -> Result<(), String> {
    spawn_watch_forwarder(app_node_manager.inner().clone(), on_event, move |node| {
        let id = mailbox_id.clone();
        async move { tracked_sync_state(&node, &id).await }
    });
    Ok(())
}
