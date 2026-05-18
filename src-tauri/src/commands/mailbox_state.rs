use std::collections::{BTreeSet, HashMap};

use dashchat_node::{topic::TopicId, DeviceId, Node};
use mailbox_client::{
    manager::{MailboxConnectionState, MailboxSyncState},
    MailboxId,
};
use serde::Serialize;
use tauri::{ipc::Channel, State};
use tokio::sync::watch;

/// Convert to the JSON-compatible shape: topic/author keys hex-encoded
/// (JSON object keys must be strings).
fn to_wire(state: &MailboxSyncState<TopicId, DeviceId>) -> MailboxSyncState<String, String> {
    let mut out: MailboxSyncState<String, String> = HashMap::new();
    for (t, authors) in state.iter() {
        let inner = out.entry(hex::encode(&**t)).or_default();
        for (a, s) in authors {
            inner.insert(hex::encode(a.as_bytes()), *s);
        }
    }
    out
}

async fn forward<T, U, F>(
    mut rx: watch::Receiver<T>,
    on_event: Channel<U>,
    transform: F,
) -> Result<(), String>
where
    T: Clone + Send + Sync + 'static,
    U: Serialize + Clone + Send + Sync + 'static,
    F: Fn(&T) -> U + Send + Sync + 'static,
{
    on_event
        .send(transform(&rx.borrow()))
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let value = transform(&rx.borrow());
            if on_event.send(value).is_err() {
                break;
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn mailbox_subscribe_ids(
    on_event: Channel<BTreeSet<MailboxId>>,
    node: State<'_, Node>,
) -> Result<(), String> {
    forward(node.mailboxes.mailbox_ids(), on_event, |s| s.clone()).await
}

#[tauri::command]
pub async fn mailbox_subscribe_connection_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxConnectionState>,
    node: State<'_, Node>,
) -> Result<(), String> {
    let tracked = node
        .mailboxes
        .tracked_mailbox(&mailbox_id)
        .await
        .ok_or_else(|| format!("unknown mailbox {mailbox_id}"))?;
    forward(tracked.connection_state(), on_event, |s| s.clone()).await
}

#[tauri::command]
pub async fn mailbox_subscribe_sync_state(
    mailbox_id: MailboxId,
    on_event: Channel<MailboxSyncState<String, String>>,
    node: State<'_, Node>,
) -> Result<(), String> {
    let tracked = node
        .mailboxes
        .tracked_mailbox(&mailbox_id)
        .await
        .ok_or_else(|| format!("unknown mailbox {mailbox_id}"))?;
    forward(tracked.sync_state(), on_event, to_wire).await
}
