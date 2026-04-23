use std::collections::{HashMap, HashSet};
use std::future::Future;

use axum::{Json, extract::State, http::StatusCode};
use futures::StreamExt;

use push_notifications_client::requests::NotifyTopicsRequest;
use push_notifications_client::types::{
    FcmToken, OperationId, PublicKey, PushNotification, TopicId,
};

use crate::{AppState, error::AppError, fcm_client::SendResult};

/// Maximum number of concurrent FCM send requests per incoming notify call.
const MAX_CONCURRENT_SENDS: usize = 50;

pub(crate) async fn notify_topics(
    State(state): State<AppState>,
    Json(req): Json<NotifyTopicsRequest>,
) -> Result<StatusCode, AppError> {
    req.validate()?;

    let topic_ids: HashSet<_> = req.topics_to_notify.keys().cloned().collect();

    // Batch-fetch subscribers for all topics in a single query
    let topic_subscribers = state.db.get_subscribers_for_topics(&topic_ids).await?;

    // Batch-fetch FCM tokens for all unique subscribers in a single query
    let all_subscribers: Vec<PublicKey> = topic_subscribers
        .values()
        .flatten()
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let fcm_tokens = state.db.get_fcm_tokens(&all_subscribers).await?;

    let tasks: Vec<_> = req
        .topics_to_notify
        .iter()
        .flat_map(|(topic_id, op_ids)| {
            notify_topic(&state, topic_id, op_ids, topic_subscribers.get(topic_id), &fcm_tokens)
        })
        .collect();

    if tasks.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }

    tracing::info!(
        topics = req.topics_to_notify.len(),
        tasks = tasks.len(),
        "notifying topic subscribers"
    );

    futures::stream::iter(tasks)
        .buffer_unordered(MAX_CONCURRENT_SENDS)
        .collect::<Vec<()>>()
        .await;

    Ok(StatusCode::NO_CONTENT)
}

fn notify_topic(
    state: &AppState,
    topic_id: &TopicId,
    op_ids: &HashSet<OperationId>,
    subscribers: Option<&Vec<PublicKey>>,
    fcm_tokens: &HashMap<PublicKey, FcmToken>,
) -> Vec<impl Future<Output = ()>> {
    let Some(subscribers) = subscribers else {
        return Vec::new();
    };

    let mut tasks = Vec::new();

    // One push per operation: iOS Notification Service Extensions can only
    // transform a single incoming notification, so each operation needs its own push
    // for the client to resolve it into a user-facing message.
    for op_id in op_ids {
        let notification = PushNotification {
            title: topic_id.to_string(),
            body: op_id.to_string(),
        };

        for public_key in subscribers {
            if let Some(fcm_token) = fcm_tokens.get(public_key) {
                tasks.push(send_notification(
                    state.clone(),
                    public_key.clone(),
                    fcm_token.clone(),
                    notification.clone(),
                ));
            }
        }
    }

    tasks
}

async fn send_notification(
    state: AppState,
    public_key: PublicKey,
    fcm_token: FcmToken,
    notification: PushNotification,
) {
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
