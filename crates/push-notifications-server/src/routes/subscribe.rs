use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::SubscribeRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn subscribe(
    State(state): State<AppState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state
        .db
        .subscribe_to_topics(&req.public_key, &req.topic_ids)
        .await?;
    tracing::info!(
        public_key = %req.public_key,
        topics = req.topic_ids.len(),
        "subscribed to topics"
    );
    Ok(StatusCode::NO_CONTENT)
}
