use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::UnsubscribeRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn unsubscribe(
    State(state): State<AppState>,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state
        .db
        .unsubscribe_from_topics(&req.public_key, &req.topic_ids)
        .await?;
    tracing::info!(
        public_key = %req.public_key,
        topics = req.topic_ids.len(),
        "unsubscribed from topics"
    );
    Ok(StatusCode::NO_CONTENT)
}
