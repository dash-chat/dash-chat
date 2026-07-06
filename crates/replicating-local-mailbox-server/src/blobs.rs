//! Blob replication support.
//!
//! `/blips/get` returns blip payloads only, never the blob hashes they reference
//! (those are passed separately, in cleartext, by the storing client, because
//! the mailbox can't read the encrypted blip). So a peer holding blips
//! replicated from us has no way to learn which blobs they reference.
//!
//! To bridge that gap, [`announce_blobs_loop`] periodically announces our full
//! blob-store hash list to every registered peer through the standard mailbox
//! API: `POST /peers/register` teaches the peer our dialing address, and
//! `POST /blobs/store` makes it record us as a fetch source for every hash it
//! doesn't already hold — its own fetch loop then downloads the content over
//! iroh, exactly as it does for hashes announced by a storing client.

use std::time::Duration;

use mailbox_server::{BlobSync, RegisterPeerRequest, StoreBlobsRequest};

use crate::item::BlipItem;
use crate::store::RedbBlipStore;
use mailbox_client::manager::Mailboxes;

/// Periodically announce every blob we hold to each registered peer; runs until
/// cancelled. Peers that already hold a hash ignore it (the `/blobs/store`
/// handler diffs against its own store), so re-announcing is idempotent.
pub async fn announce_blobs_loop(
    mailboxes: Mailboxes<BlipItem, RedbBlipStore>,
    blob_sync: BlobSync,
    interval: Duration,
) {
    let client = reqwest::Client::new();
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let hashes = match blob_sync.blobs.store().blobs().list().hashes().await {
            Ok(hashes) => hashes,
            Err(err) => {
                tracing::error!("failed to list blobs: {err}");
                continue;
            }
        };
        if hashes.is_empty() {
            continue;
        }
        let ids: Vec<String> = mailboxes
            .active_mailbox_ids()
            .borrow()
            .iter()
            .cloned()
            .collect();
        for id in ids {
            let Some(tracked) = mailboxes.tracked_mailbox(&id).await else {
                continue;
            };
            let Some(url) = tracked.client().await.url() else {
                continue;
            };
            if let Err(err) = announce_blobs_to_peer(&client, &url, &hashes, &blob_sync).await {
                tracing::debug!("blob-sync: announce to peer {id} failed: {err}");
            }
        }
    }
}

/// Announce `hashes` to the peer at `peer_url`: register our dialing address so
/// the peer can reach us, then announce the hashes with our endpoint id as the
/// source its fetch pool should dial for whatever it lacks.
pub async fn announce_blobs_to_peer(
    client: &reqwest::Client,
    peer_url: &str,
    hashes: &[iroh_blobs::Hash],
    blob_sync: &BlobSync,
) -> anyhow::Result<()> {
    client
        .post(format!("{peer_url}/peers/register"))
        .json(&RegisterPeerRequest {
            addr: blob_sync.endpoint_addr(),
        })
        .send()
        .await?
        .error_for_status()?;
    client
        .post(format!("{peer_url}/blobs/store"))
        .json(&StoreBlobsRequest {
            blob_hashes: hashes.to_vec(),
            sender_pubkey: blob_sync.endpoint_id(),
            signature: Vec::new(),
        })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
