//! Wire-compatibility shim for Dash Chat 0.18.x clients.
//!
//! 0.18.x speaks the pre-rename mailbox protocol: `POST /blobs/store` and
//! `POST /blobs/get`, where "blob" meant what is now a "blip" (an opaque
//! base64 payload carried inline in the JSON body — iroh blob transfer didn't
//! exist yet). The 0.19 rename to `/blips/*` left old clients 404ing with no
//! version negotiation to tell them why, so the old routes stay served as
//! field-rename adapters over the same store and handlers.
//!
//! Scope: this bridges inline blip payloads only. Media that 0.19 clients
//! reference as iroh blob hashes has no inline representation for a 0.18
//! client to fetch, so it won't round-trip across versions.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    get_blips_for_topics, store_blips, AppState, Author, Blip, GetBlipsRequest, SequenceNumber,
    StoreBlipsRequest, TopicId,
};

/// The 0.18 `POST /blobs/store` body: a [`StoreBlipsRequest`] under its 0.18
/// field name. 0.18 had no blob hashes, sender pubkey, or signature.
#[derive(Deserialize)]
struct StoreBlobsV018Request {
    blobs: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
}

/// The 0.18 `POST /blobs/get` response:
/// [`GetBlipsResponse`](crate::GetBlipsResponse) under its 0.18 field names.
#[derive(Serialize)]
struct GetBlobsV018Response {
    blobs_by_topic: BTreeMap<TopicId, GetBlobsForTopicV018Response>,
}

#[derive(Serialize)]
struct GetBlobsForTopicV018Response {
    blobs: BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>,
    missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

/// `/blobs/store` serves two protocols on one path: the current blob-hash
/// announcement and the 0.18 inline-blip store. Dispatch by body shape: the
/// current body carries `blob_hashes`/`sender_pubkey`, the 0.18 body a `blobs`
/// map — the shapes are disjoint (no defaulted required fields overlap).
pub(crate) async fn store_blobs_dispatch(
    state: State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, String)> {
    if let Ok(request) = serde_json::from_value::<crate::StoreBlobsRequest>(payload.clone()) {
        return Ok(crate::store_blobs(state, Json(request))
            .await
            .into_response());
    }
    let request: StoreBlobsV018Request = serde_json::from_value(payload)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    store_blips(
        state,
        Json(StoreBlipsRequest {
            blips: request.blobs,
            sender_pubkey: None,
            signature: vec![],
        }),
    )
    .await
    .map(|status| status.into_response())
}

// The 0.18 request shape (`{"topics": ...}`) is identical to today's
// GetBlipsRequest; only the response fields were renamed.
pub(crate) async fn get_blobs_v018(
    state: State<AppState>,
    payload: Json<GetBlipsRequest>,
) -> Result<Response, (StatusCode, String)> {
    let Json(response) = get_blips_for_topics(state, payload).await?;
    Ok(Json(GetBlobsV018Response {
        blobs_by_topic: response
            .blips_by_topic
            .into_iter()
            .map(|(topic, t)| {
                (
                    topic,
                    GetBlobsForTopicV018Response {
                        blobs: t.blips,
                        missing: t.missing,
                    },
                )
            })
            .collect(),
    })
    .into_response())
}
