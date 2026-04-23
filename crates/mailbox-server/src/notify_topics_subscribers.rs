use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{OperationId, TopicId as PushTopicId};

use crate::{AppState, TopicId};

/// Notify push notification subscribers for topics that received new data (non-blocking).
pub async fn notify_topics_subscribers(
    state: &AppState,
    topics_with_new_blobs: BTreeMap<TopicId, BTreeSet<String>>,
) {
    if topics_with_new_blobs.is_empty() {
        return;
    }
    let Some(push_client) = &state.push_client else {
        return;
    };
    let push_client = push_client.clone();
    let mut push_tasks = state.push_tasks.lock().await;
    // Reap completed tasks to prevent slow memory leak
    while push_tasks.try_join_next().is_some() {}
    push_tasks.spawn(send_push_notifications(push_client, topics_with_new_blobs));
}

async fn send_push_notifications(
    push_client: Arc<PushNotificationsClient>,
    topics_with_new_blobs: BTreeMap<TopicId, BTreeSet<String>>,
) {
    let topics_to_notify: HashMap<PushTopicId, HashSet<OperationId>> = topics_with_new_blobs
        .into_iter()
        .map(|(topic, ops)| {
            let topic = PushTopicId::from(topic);
            let ops = ops.into_iter().map(OperationId::from).collect();
            (topic, ops)
        })
        .collect();

    if let Err(e) = dashchat_utils::retry_with_backoff(
        Some(5),
        Duration::from_secs(1),
        Duration::from_secs(60),
        "notify push subscribers",
        || push_client.notify_topics(topics_to_notify.clone()),
    )
    .await
    {
        tracing::error!("Failed to notify push subscribers: {e:#}");
    } else {
        tracing::info!("Notified push subscribers for topics.");
    }
}
