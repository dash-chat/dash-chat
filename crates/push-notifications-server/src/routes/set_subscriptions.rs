use axum::{extract::State, http::StatusCode, Json};

use push_notifications_client::requests::SetSubscriptionsRequest;

use crate::{error::AppError, AppState};

pub(crate) async fn set_subscriptions(
    State(state): State<AppState>,
    Json(req): Json<SetSubscriptionsRequest>,
) -> Result<StatusCode, AppError> {
    state
        .db
        .set_subscriptions(&req.public_key, &req.topic_ids)
        .await?;
    tracing::info!(
        public_key = %req.public_key,
        topics = req.topic_ids.len(),
        "set subscriptions"
    );
    Ok(StatusCode::NO_CONTENT)
}
