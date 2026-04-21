use axum::{Json, extract::State, http::StatusCode};
use futures::future::join_all;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AppState,
    error::AppError,
    types::{OperationId, PushNotification, TopicId},
};

#[derive(Deserialize, serde::Serialize)]
pub struct NotifyTopicsRequest {
    pub topics_to_notify: BTreeMap<TopicId, BTreeSet<OperationId>>,
}

pub(crate) async fn notify_topics(
    State(state): State<AppState>,
    Json(req): Json<NotifyTopicsRequest>,
) -> Result<StatusCode, AppError> {
    let mut tasks = Vec::new();

    for (topic_id, op_ids) in &req.topics_to_notify {
        let subscribers = match state.db.get_subscribers(topic_id).await {
            Ok(subs) => subs,
            Err(e) => {
                tracing::warn!(topic_id = %topic_id, "failed to get subscribers: {e:#}");
                continue;
            }
        };

        for op_id in op_ids {
            let notification = PushNotification {
                title: topic_id.to_string(),
                body: op_id.to_string(),
            };

            for public_key in &subscribers {
                let state = state.clone();
                let public_key = public_key.clone();
                let notification = notification.clone();
                tasks.push(async move {
                    match state.db.get_fcm_token(&public_key).await {
                        Ok(Some(fcm_token)) => {
                            if let Err(e) = state
                                .fcm
                                .send_push_notification(&fcm_token, &notification)
                                .await
                            {
                                tracing::warn!(public_key = %public_key, "failed to send FCM notification: {e:#}");
                            }
                        }
                        Ok(None) => {
                            tracing::debug!(public_key = %public_key, "no FCM token registered, skipping");
                        }
                        Err(e) => {
                            tracing::warn!(public_key = %public_key, "failed to look up FCM token: {e:#}");
                        }
                    }
                });
            }
        }
    }

    if tasks.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }

    tracing::info!(
        topics = req.topics_to_notify.len(),
        tasks = tasks.len(),
        "notifying topic subscribers"
    );

    join_all(tasks).await;

    Ok(StatusCode::NO_CONTENT)
}
