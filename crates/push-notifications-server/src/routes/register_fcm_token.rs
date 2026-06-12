use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::RegisterFcmTokenRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn register_fcm_token(
    State(state): State<AppState>,
    Json(req): Json<RegisterFcmTokenRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state.db.store_fcm_token(&req.verifying_key, &req.fcm_token).await?;
    tracing::info!(verifying_key = %req.verifying_key, "stored FCM token");
    Ok(StatusCode::NO_CONTENT)
}
