//! Wire-compatibility shim for Dash Chat 0.18.x clients.
//!
//! 0.18.x speaks the pre-rename mailbox protocol: `POST /blobs/store` and
//! `POST /blobs/get`, where "blob" meant what is now a "blip" (an opaque
//! base64 payload carried inline in the JSON body — iroh blob transfer didn't
//! exist yet). The 0.19 rename to `/blips/*` left old clients 404ing with no
//! version negotiation to tell them why, so the LAN appliance re-exposes the
//! old routes as field-rename adapters over the same store and handlers.
//!
//! Scope: this bridges inline blip payloads only. Media that 0.19 clients
//! reference as iroh blob hashes has no inline representation for a 0.18
//! client to fetch, so it won't round-trip across versions.

use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use mailbox_server::{
    get_blips_for_topics, store_blips, AppState, Author, Blip, GetBlipsRequest, SequenceNumber,
    StoreBlipsRequest, TopicId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Mirrors the main router's MAX_PAYLOAD_SIZE: routers merged as `extra` don't
// inherit its layers, and 0.18 clients carry attachments inline (up to the
// UI's 16 MiB cap, ~1.33x that as base64).
const MAX_PAYLOAD_SIZE: usize = 64 * 1024 * 1024; // 64 MB

/// `POST /blobs/store` body: a [`StoreBlipsRequest`] under its 0.18 field
/// name. 0.18 had no blob hashes, sender pubkey, or signature.
#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blobs: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
}

/// `POST /blobs/get` response: [`GetBlipsResponse`](mailbox_server::GetBlipsResponse)
/// under its 0.18 field names.
#[derive(Serialize, Deserialize)]
pub struct GetBlobsResponse {
    pub blobs_by_topic: BTreeMap<TopicId, GetBlobsForTopicResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsForTopicResponse {
    pub blobs: BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>,
    pub missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

/// A router exposing the 0.18-era `/blobs/store` and `/blobs/get`, merged into
/// the appliance's extra router alongside `/blobs/list` (no path overlap).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/blobs/store", post(store_blobs_v018))
        .route("/blobs/get", post(get_blobs_v018))
        .layer(DefaultBodyLimit::max(MAX_PAYLOAD_SIZE))
}

async fn store_blobs_v018(
    state: State<AppState>,
    Json(payload): Json<StoreBlobsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    store_blips(
        state,
        Json(StoreBlipsRequest {
            blips: payload.blobs,
            blob_hashes: vec![],
            sender_pubkey: None,
            signature: vec![],
        }),
    )
    .await
}

// The 0.18 request shape (`{"topics": ...}`) is identical to today's
// GetBlipsRequest; only the response fields were renamed.
async fn get_blobs_v018(
    state: State<AppState>,
    payload: Json<GetBlipsRequest>,
) -> Result<Json<GetBlobsResponse>, (StatusCode, String)> {
    let Json(response) = get_blips_for_topics(state, payload).await?;
    Ok(Json(GetBlobsResponse {
        blobs_by_topic: response
            .blips_by_topic
            .into_iter()
            .map(|(topic, t)| {
                (
                    topic,
                    GetBlobsForTopicResponse {
                        blobs: t.blips,
                        missing: t.missing,
                    },
                )
            })
            .collect(),
    }))
}
