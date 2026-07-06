//! Persist pulled items. The sync engine delivers items it fetched from peers on
//! the `subscribe(topic)` channel; it does NOT store them. We POST them back to
//! the mailbox's own `/blips/store` so watermark recomputation and subscriber
//! notification stay centralized in the one handler.

use std::collections::BTreeMap;

use mailbox_server::{Blip, StoreBlipsRequest};
use tokio::sync::mpsc;

use crate::item::BlipItem;

/// Drain `rx` (pulled items for one topic) and store each batch via
/// `self_store_url` (the mailbox's own `/blips/store`); runs until cancelled
/// or the channel closes.
pub async fn drain_loop(mut rx: mpsc::Receiver<BlipItem>, self_store_url: String) {
    let client = reqwest::Client::new();
    while let Some(first) = rx.recv().await {
        // Opportunistically batch anything already queued.
        let mut items = vec![first];
        while let Ok(item) = rx.try_recv() {
            items.push(item);
        }
        if let Err(err) = store_items(&client, &self_store_url, items).await {
            tracing::warn!("drain: failed to store pulled blips: {err}");
        }
    }
}

async fn store_items(
    client: &reqwest::Client,
    url: &str,
    items: Vec<BlipItem>,
) -> anyhow::Result<()> {
    let mut blips: BTreeMap<String, BTreeMap<String, BTreeMap<u64, Blip>>> = BTreeMap::new();
    for item in items {
        blips
            .entry(item.key.topic_id)
            .or_default()
            .entry(item.key.author)
            .or_default()
            .insert(item.key.sequence_number, item.blip);
    }
    // The synthesized uuid is discarded; `/blips/store` mints its own. No blob
    // hashes: opaque blips carry none, and blobs are handled by the blob loop.
    let request = StoreBlipsRequest {
        blips,

        sender_pubkey: None,
        signature: vec![],
    };
    client
        .post(url)
        .json(&request)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
