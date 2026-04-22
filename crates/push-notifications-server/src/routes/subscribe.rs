use axum::{extract::State, http::StatusCode, Json};

use push_notifications_client::requests::SubscribeRequest;

use crate::{error::AppError, AppState};

pub(crate) async fn subscribe(
    State(state): State<AppState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<StatusCode, AppError> {
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
