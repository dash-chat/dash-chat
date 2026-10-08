use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use dashchat_node::topic::TopicId;
use dashchat_node::Node;
use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{TopicId as PushTopicId, VerifyingKey};
use tauri::{AppHandle, EventId, Listener};
use tokio::sync::{mpsc, watch, Notify};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::NOTIFICATIONS_ENABLED_UPDATED_EVENT;
use crate::settings::load_settings_from_data_dir;

/// How long to wait for more changes before acting on one, so a burst (every
/// stored topic, when the node starts) is handled in one pass.
const CHANGES_DEBOUNCE: Duration = Duration::from_millis(500);

/// Keeps the push notifications server subscribed, for this device, to every
/// topic of a node: the server only wakes the device for those. Holds a clone
/// of the node, so it must be shut down before the node is.
#[derive(Clone)]
pub(crate) struct SubscribeToPushNotificationsForTopicsTask {
    app_handle: Option<AppHandle>,
    notifications_enabled_listener: Option<EventId>,
    pub(crate) topics_on_server: watch::Receiver<HashSet<PushTopicId>>,
    tracker: TaskTracker,
    token: CancellationToken,
}

impl SubscribeToPushNotificationsForTopicsTask {
    /// `new_topics` receives each topic the node subscribes to. `app_handle`,
    /// when there is one, is listened to for changes of the notifications
    /// setting; the setting itself is read from the node's data directory.
    pub(crate) fn spawn(
        node: Node,
        app_handle: Option<AppHandle>,
        client: PushNotificationsClient,
        new_topics: mpsc::Receiver<TopicId>,
    ) -> Self {
        let changed = Arc::new(Notify::new());

        let notifications_enabled_changed = changed.clone();
        let notifications_enabled_listener = app_handle.as_ref().map(|handle| {
            handle.listen(NOTIFICATIONS_ENABLED_UPDATED_EVENT, move |_event| {
                notifications_enabled_changed.notify_one();
            })
        });

        let (topics_on_server_tx, topics_on_server) = watch::channel(HashSet::new());
        let tracker = TaskTracker::new();
        let token = CancellationToken::new();
        tracker.spawn(
            token
                .clone()
                .run_until_cancelled_owned(keep_topics_subscribed(
                    node,
                    client,
                    new_topics,
                    changed,
                    topics_on_server_tx,
                )),
        );
        Self {
            app_handle,
            notifications_enabled_listener,
            topics_on_server,
            tracker,
            token,
        }
    }

    /// Stop, and wait until stopped.
    pub(crate) async fn shutdown(&self) {
        if let (Some(handle), Some(listener)) =
            (&self.app_handle, self.notifications_enabled_listener)
        {
            handle.unlisten(listener);
        }
        self.token.cancel();
        self.tracker.close();
        self.tracker.wait().await;
    }
}

async fn keep_topics_subscribed(
    node: Node,
    client: PushNotificationsClient,
    mut new_topics: mpsc::Receiver<TopicId>,
    changed: Arc<Notify>,
    topics_on_server_tx: watch::Sender<HashSet<PushTopicId>>,
) {
    let mut topics_on_server = None;
    loop {
        let update_subscriptions =
            || update_subscriptions(&node, &client, topics_on_server.as_ref());
        let result = tokio::select! {
            result = update_subscriptions
                .retry(ExponentialBuilder::new().with_jitter().without_max_times())
                .notify(|err, delay| {
                    log::warn!("Failed to update push notification subscriptions, retrying in {delay:?}: {err:?}")
                }) => result,
            // A change restarts the attempt at once, not after its backoff.
            () = changed.notified() => continue,
        };
        match result {
            Ok(topics) => {
                topics_on_server_tx.send_replace(topics.clone());
                topics_on_server = Some(topics);
            }
            Err(err) => log::warn!("Gave up updating push notification subscriptions: {err:?}"),
        }
        tokio::select! {
            () = changed.notified() => {}
            Some(_) = new_topics.recv() => {}
        }
        tokio::time::sleep(CHANGES_DEBOUNCE).await;
        while new_topics.try_recv().is_ok() {}
    }
}

/// Adds just the missing topics when the server has nothing it shouldn't;
/// otherwise replaces its whole set. Returns what the server now has.
async fn update_subscriptions(
    node: &Node,
    client: &PushNotificationsClient,
    topics_on_server: Option<&HashSet<PushTopicId>>,
) -> anyhow::Result<HashSet<PushTopicId>> {
    let topics = topics_to_subscribe_to(node).await?;
    let device = VerifyingKey::from(node.device_id().to_string());
    match topics_on_server {
        Some(on_server) if on_server.is_subset(&topics) => {
            let missing: HashSet<PushTopicId> = topics.difference(on_server).cloned().collect();
            if !missing.is_empty() {
                log::info!(
                    "Subscribing to {} topics on push notifications server.",
                    missing.len()
                );
                client.add_topic_subscriptions(device, missing).await?;
            }
        }
        _ => {
            log::info!(
                "Syncing {} topic subscriptions with push notifications server.",
                topics.len()
            );
            client
                .update_topic_subscriptions(device, topics.clone())
                .await?;
        }
    }
    Ok(topics)
}

/// Every topic the node listens to, or none while notifications are disabled.
async fn topics_to_subscribe_to(node: &Node) -> anyhow::Result<HashSet<PushTopicId>> {
    if !load_settings_from_data_dir(node.data_path()).notifications_enabled {
        return Ok(HashSet::new());
    }
    push_notifications_topics(node).await
}

async fn push_notifications_topics(node: &Node) -> anyhow::Result<HashSet<PushTopicId>> {
    // What the node listens to right now includes topics the stores do not
    // list, like the inbox a contact replies to a request on; the stores cover
    // what it has not subscribed to yet while it starts.
    let listened_to_topics = node.mailboxes.subscribed_topics().await;
    let stored_topics = node.subscribed_topics().await?;
    let inbox_topics = node
        .get_active_inbox_topics()
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .into_iter()
        .map(|inbox| *inbox.topic);
    Ok(listened_to_topics
        .into_iter()
        .chain(stored_topics)
        .chain(inbox_topics)
        .map(|topic| PushTopicId::from(topic.to_hex()))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::future::Future;

    use dashchat_node::NodeConfig;
    use push_notifications_server::driver::Driver;
    use push_notifications_server::test_utils::{SubscriptionRequest, TestPushServer};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn replaces_stale_subscriptions_with_the_nodes_topics_on_start() {
        let server = TestPushServer::start().await;
        let (node, new_topics, _dir) = start_node().await;
        let stale_topic = PushTopicId::from("stale".to_string());
        server
            .db
            .add_topic_subscriptions(&device(&node), &HashSet::from([stale_topic.clone()]))
            .await
            .unwrap();

        let _subscriptions = SubscribeToPushNotificationsForTopicsTask::spawn(
            node.clone(),
            None,
            server.client(),
            new_topics,
        );

        let expected = push_notifications_topics(&node).await.unwrap();
        eventually("the server has exactly the node's topics", || async {
            server.db.topics_of(&device(&node)) == expected
        })
        .await;
        assert_eq!(
            server.db.requests().last(),
            Some(&SubscriptionRequest::Replace(expected))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn adds_a_topic_the_node_subscribes_to_without_replacing() {
        let server = TestPushServer::start().await;
        let (node, new_topics, _dir) = start_node().await;
        let _subscriptions = SubscribeToPushNotificationsForTopicsTask::spawn(
            node.clone(),
            None,
            server.client(),
            new_topics,
        );
        wait_until_synced(&server, &node).await;

        // A replace would drop this; an add keeps it.
        let other_topic = PushTopicId::from("added elsewhere".to_string());
        server
            .db
            .add_topic_subscriptions(&device(&node), &HashSet::from([other_topic.clone()]))
            .await
            .unwrap();
        let group = node.create_group(BTreeMap::new()).await.unwrap();

        let group_topic = PushTopicId::from(group.to_hex());
        eventually("the server has the group's topic", || async {
            server.db.topics_of(&device(&node)).contains(&group_topic)
        })
        .await;
        assert!(server.db.topics_of(&device(&node)).contains(&other_topic));
        assert!(matches!(
            server.db.requests().last(),
            Some(SubscriptionRequest::Add(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publishes_a_new_topic_once_it_is_on_the_server() {
        let server = TestPushServer::start().await;
        let (node, new_topics, _dir) = start_node().await;
        let subscriptions = SubscribeToPushNotificationsForTopicsTask::spawn(
            node.clone(),
            None,
            server.client(),
            new_topics,
        );

        let group = node.create_group(BTreeMap::new()).await.unwrap();
        let group_topic = PushTopicId::from(group.to_hex());
        let mut topics_on_server = subscriptions.topics_on_server.clone();
        tokio::time::timeout(
            Duration::from_secs(15),
            topics_on_server.wait_for(|topics| topics.contains(&group_topic)),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(server.db.topics_of(&device(&node)).contains(&group_topic));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retries_until_the_server_is_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let (node, new_topics, _dir) = start_node().await;
        let client = PushNotificationsClient::new(url.clone()).unwrap();
        let _subscriptions = SubscribeToPushNotificationsForTopicsTask::spawn(
            node.clone(),
            None,
            client,
            new_topics,
        );

        tokio::time::sleep(Duration::from_millis(500)).await;
        let server = TestPushServer::start_at(
            TcpListener::bind(url.trim_start_matches("http://"))
                .await
                .unwrap(),
        )
        .await;

        wait_until_synced(&server, &node).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sends_nothing_after_shutdown() {
        let server = TestPushServer::start().await;
        let (node, new_topics, _dir) = start_node().await;
        let subscriptions = SubscribeToPushNotificationsForTopicsTask::spawn(
            node.clone(),
            None,
            server.client(),
            new_topics,
        );
        wait_until_synced(&server, &node).await;

        subscriptions.shutdown().await;
        let group = node.create_group(BTreeMap::new()).await.unwrap();
        tokio::time::sleep(CHANGES_DEBOUNCE * 3).await;

        let group_topic = PushTopicId::from(group.to_hex());
        assert!(!server.db.topics_of(&device(&node)).contains(&group_topic));
    }

    /// A fresh node; the channel it sends each topic it subscribes to on; its store,
    /// which is deleted when dropped.
    async fn start_node() -> (Node, mpsc::Receiver<TopicId>, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (sender, new_topics) = mpsc::channel(100);
        let node = Node::new(dir.path().into(), NodeConfig::testing(), None, Some(sender))
            .await
            .unwrap();
        (node, new_topics, dir)
    }

    fn device(node: &Node) -> VerifyingKey {
        VerifyingKey::from(node.device_id().to_string())
    }

    async fn wait_until_synced(server: &TestPushServer, node: &Node) {
        let expected = push_notifications_topics(node).await.unwrap();
        eventually("the server has the node's topics", || async {
            server.db.topics_of(&device(node)) == expected
        })
        .await;
    }

    async fn eventually<F: Future<Output = bool>>(what: &str, check: impl Fn() -> F) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !check().await {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting until {what}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
