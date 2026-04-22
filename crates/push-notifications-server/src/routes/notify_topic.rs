use axum::{Json, extract::State, http::StatusCode};
use futures::future::join_all;

use push_notifications_client::requests::NotifyTopicsRequest;
use push_notifications_client::types::PushNotification;

use crate::{AppState, error::AppError, fcm_client::SendResult};

pub(crate) async fn notify_topics(
    State(state): State<AppState>,
    Json(req): Json<NotifyTopicsRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;
    let mut tasks = Vec::new();

    for (topic_id, op_ids) in &req.topics_to_notify {
        let subscribers = match state.db.get_subscribers(topic_id).await {
            Ok(subs) => subs,
            Err(e) => {
                tracing::warn!(topic_id = %topic_id, "failed to get subscribers: {e:#}");
                continue;
            }
        };

        // One push per operation: iOS Notification Service Extensions can only
        // transform a single incoming notification, so each operation needs its own push
        // for the client to resolve it into a user-facing message.
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
                            match state
                                .fcm
                                .send_push_notification(&fcm_token, &notification)
                                .await
                            {
                                SendResult::Ok => {}
                                SendResult::InvalidToken => {
                                    tracing::info!(public_key = %public_key, "FCM token is invalid, removing");
                                    if let Err(e) = state.db.remove_fcm_token(&public_key).await {
                                        tracing::warn!(public_key = %public_key, "failed to remove invalid FCM token: {e:#}");
                                    }
                                }
                                SendResult::Error(e) => {
                                    tracing::warn!(public_key = %public_key, "failed to send FCM notification: {e}");
                                }
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
