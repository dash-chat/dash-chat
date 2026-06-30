use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct RegisterPeerRequest {
    pub addr: iroh::EndpointAddr,
}

pub async fn register_peer(
    State(state): State<AppState>,
    Json(payload): Json<RegisterPeerRequest>,
) -> StatusCode {
    state.blob_sync.add_peer_addr(payload.addr);
    StatusCode::NO_CONTENT
}
