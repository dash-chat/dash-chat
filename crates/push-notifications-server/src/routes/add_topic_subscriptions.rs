use axum::{Json, extract::State, http::StatusCode};

use push_notifications_client::requests::AddTopicSubscriptionsRequest;

use crate::{AppState, error::AppError};

pub(crate) async fn add_topic_subscriptions(
    State(state): State<AppState>,
    Json(req): Json<AddTopicSubscriptionsRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    state
        .db
        .add_topic_subscriptions(&req.verifying_key, &req.topic_ids)
        .await?;
    tracing::info!(
        verifying_key = %req.verifying_key,
        topics = req.topic_ids.len(),
        "subscribed to topics"
    );
    Ok(StatusCode::NO_CONTENT)
}
