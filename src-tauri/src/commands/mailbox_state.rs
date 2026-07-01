use std::collections::BTreeSet;

use dashchat_node::{topic::TopicId, DeviceId};
use mailbox_client::{manager::MailboxConnectionState, sync_tracker::MailboxSyncState, MailboxId};
use serde::Serialize;
use tauri::{ipc::Channel, State};
use tokio::sync::watch;

use crate::app_node::AppNode;

async fn forward<T>(mut rx: watch::Receiver<T>, on_event: Channel<T>) -> Result<(), String>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    on_event
        .send(rx.borrow().clone())
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let value = rx.borrow().clone();
            if on_event.send(value).is_err() {
                break;
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_active_ids(
    on_event: Channel<BTreeSet<MailboxId>>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    forward(node.mailboxes.active_mailbox_ids(), on_event).await
}

#[tauri::command]
pub async fn mailbox_subscribe_cloud_id(
    on_event: Channel<Option<MailboxId>>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    let rx = dashchat_utils::derive_watch(node.mailboxes.active_mailbox_ids(), move |_ids| {
        let node = node.clone();
        async move { crate::mailbox::cloud_mailbox_id(&node).await }
    })
    .await;
    forward(rx, on_event).await
}

#[tauri::command]
pub async fn mailbox_subscribe_all_ids(
    on_event: Channel<BTreeSet<MailboxId>>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    forward(node.mailboxes.sync_tracker().all_mailbox_ids(), on_event).await
}

#[tauri::command]
pub async fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxConnectionState>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    let mailbox = node
        .mailboxes
        .tracked_mailbox(&mailbox_id)
        .await
        .ok_or_else(|| format!("unknown mailbox {mailbox_id}"))?;
    forward(mailbox.connection_state(), on_event).await
}

#[tauri::command]
pub async fn mailbox_subscribe_sync_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxSyncState<TopicId, DeviceId>>,
    app_node: State<'_, AppNode>,
) -> Result<(), String> {
    let node = app_node.get().await?;
    let rx = node
        .mailboxes
        .sync_tracker()
        .sync_state(&mailbox_id)
        .await
        .map_err(|e| e.to_string())?;
    forward(rx, on_event).await
}
