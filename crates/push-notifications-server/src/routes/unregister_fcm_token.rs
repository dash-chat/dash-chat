use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::UnregisterFcmTokenRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn unregister_fcm_token(
    State(state): State<AppState>,
    Json(req): Json<UnregisterFcmTokenRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state.db.remove_fcm_token(&req.public_key).await?;
    tracing::info!(public_key = %req.public_key, "unregistered FCM token");
    Ok(StatusCode::NO_CONTENT)
}
