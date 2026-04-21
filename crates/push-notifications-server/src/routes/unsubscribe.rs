use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::{
    error::AppError,
    types::{PublicKey, TopicId},
    AppState,
};

#[derive(Deserialize, serde::Serialize)]
pub struct UnsubscribeRequest {
    pub public_key: PublicKey,
    pub topic_ids: Vec<TopicId>,
}

pub(crate) async fn unsubscribe(
    State(state): State<AppState>,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<StatusCode, AppError> {
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
