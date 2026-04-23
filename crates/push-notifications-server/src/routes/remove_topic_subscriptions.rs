use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::RemoveTopicSubscriptionsRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn remove_topic_subscriptions(
    State(state): State<AppState>,
    Json(req): Json<RemoveTopicSubscriptionsRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state
        .db
        .remove_topic_subscriptions(&req.public_key, &req.topic_ids)
        .await?;
    tracing::info!(
        public_key = %req.public_key,
        topics = req.topic_ids.len(),
        "unsubscribed from topics"
    );
    Ok(StatusCode::NO_CONTENT)
}
