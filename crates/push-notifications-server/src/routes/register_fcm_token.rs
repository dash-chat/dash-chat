use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::RegisterFcmTokenRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn register_fcm_token(
    State(state): State<AppState>,
    Json(req): Json<RegisterFcmTokenRequest>,
) -> Result<StatusCode, AppError> {
    state
        .db
        .store_fcm_token(&req.public_key, &req.fcm_token)
        .await?;
    tracing::info!(public_key = %req.public_key, "stored FCM token");
    Ok(StatusCode::NO_CONTENT)
}
