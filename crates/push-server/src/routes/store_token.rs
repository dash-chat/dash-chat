use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;

use crate::{
    AppState,
    error::AppError,
    types::{FcmToken, PublicKey},
};

#[derive(Deserialize)]
pub struct StoreTokenRequest {
    pub public_key: PublicKey,
    pub fcm_token: FcmToken,
}

pub async fn store_token(
    State(state): State<AppState>,
    Json(req): Json<StoreTokenRequest>,
) -> Result<StatusCode, AppError> {
    state
        .db
        .store_fcm_token(&req.public_key, &req.fcm_token)
        .await?;
    tracing::info!(public_key = %req.public_key, "stored FCM token");
    Ok(StatusCode::NO_CONTENT)
}
