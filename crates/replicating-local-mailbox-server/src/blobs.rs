//! Blob replication support.
//!
//! `/blips/get` returns blip payloads only, never the blob hashes they reference
//! (those are passed separately, in cleartext, by the storing client, because
//! the mailbox can't read the encrypted blip). So a replicating mailbox has no
//! way to learn which blobs to fetch from a peer.
//!
//! To bridge that gap we serve a small **`/blobs/list`** endpoint that
//! enumerates the iroh blob store (and advertises our iroh address).
//! [`mirror_peer_blobs_loop`] periodically queries each registered peer's list,
//! teaches iroh the peer's direct address, and adds the peer as a fetch source
//! for any blob we don't already have; the existing `BlobSync` fetch loop then
//! downloads the content over iroh.
//!
//! A peer that doesn't expose `/blobs/list` (e.g. the cloud mailbox or the
//! app's in-process mailbox) returns 404 and is skipped — its blobs still
//! resolve to clients peer-to-peer.

use std::collections::HashSet;
use std::time::Duration;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use mailbox_server::{decode_mailbox_id, AppState, BlobSync};
use serde::{Deserialize, Serialize};

use crate::item::BlipItem;
use crate::store::RedbBlipStore;
use mailbox_client::manager::Mailboxes;

#[derive(Serialize, Deserialize)]
pub struct BlobList {
    pub hashes: Vec<iroh_blobs::Hash>,
    /// Our iroh address (id + direct/relay addresses) so a peer can dial us to
    /// fetch blobs directly — including over a LAN without n0 DNS discovery.
    #[serde(default)]
    pub addr: Option<iroh::EndpointAddr>,
}

/// A router exposing `/blobs/list`, passed to
/// [`LocalMailboxServer::spawn`](mailbox_local_server::LocalMailboxServer::spawn)
/// as the extra router (which supplies the state).
pub fn router() -> Router<AppState> {
    Router::new().route("/blobs/list", get(list_blobs))
}

async fn list_blobs(State(blob_sync): State<BlobSync>) -> Result<Json<BlobList>, StatusCode> {
    match blob_sync.blobs.store().blobs().list().hashes().await {
        Ok(hashes) => Ok(Json(BlobList {
            hashes,
            addr: Some(blob_sync.endpoint_addr()),
        })),
        Err(err) => {
            tracing::error!("failed to list blobs: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Ask the peer at `peer_url` for its blob hashes and return those we don't
/// already hold. Also teaches iroh the peer's direct address so the fetch works
/// without n0 DNS discovery. Returns an empty list if the peer doesn't expose
/// `/blobs/list` (e.g. the cloud mailbox).
pub async fn peer_blob_hashes_to_fetch(
    client: &reqwest::Client,
    peer_url: &str,
    blob_sync: &BlobSync,
) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
    let resp = client.get(format!("{peer_url}/blobs/list")).send().await?;
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }
    let list: BlobList = resp.json().await?;
    if let Some(addr) = list.addr {
        blob_sync.add_peer_addr(addr);
    }
    let held: HashSet<iroh_blobs::Hash> = blob_sync
        .blobs
        .store()
        .blobs()
        .list()
        .hashes()
        .await?
        .into_iter()
        .collect();
    Ok(list
        .hashes
        .into_iter()
        .filter(|hash| !held.contains(hash))
        .collect())
}

/// Periodically ask each registered peer for its blob list and register the
/// peer as a fetch source for any blob we lack; runs until cancelled.
pub async fn mirror_peer_blobs_loop(
    mailboxes: Mailboxes<BlipItem, RedbBlipStore>,
    blob_sync: BlobSync,
    interval: Duration,
) {
    let client = reqwest::Client::new();
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let ids: Vec<String> = mailboxes
            .active_mailbox_ids()
            .borrow()
            .iter()
            .cloned()
            .collect();
        for id in ids {
            let Ok(endpoint_id) = decode_mailbox_id(&id) else {
                continue;
            };
            let Some(tracked) = mailboxes.tracked_mailbox(&id).await else {
                continue;
            };
            let Some(url) = tracked.client().await.url() else {
                continue;
            };
            match peer_blob_hashes_to_fetch(&client, &url, &blob_sync).await {
                Ok(hashes) => {
                    for hash in hashes {
                        blob_sync.fetch_pool().add_source(hash, endpoint_id).await;
                    }
                }
                Err(err) => tracing::debug!("blob-sync: peer {id} list failed: {err}"),
            }
        }
    }
}
