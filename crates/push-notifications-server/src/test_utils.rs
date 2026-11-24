//! An in-process push notifications server for tests of its clients.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use push_notifications_client::client::PushNotificationsClient;
use push_notifications_client::types::{FcmToken, TopicId, VerifyingKey};
use tokio::net::TcpListener;

use crate::driver::Driver;
use crate::fcm_client::MockFcm;

/// A push notifications server running in this process, whose subscriptions
/// and requests tests can inspect through `db`. It never sends a push.
pub struct TestPushServer {
    pub db: Arc<RecordingDb>,
    pub url: String,
}

impl TestPushServer {
    pub async fn start() -> Self {
        Self::start_at(TcpListener::bind("127.0.0.1:0").await.unwrap()).await
    }

    /// Serve on `listener`, e.g. one bound to an address a client already uses.
    pub async fn start_at(listener: TcpListener) -> Self {
        let db = Arc::new(RecordingDb::default());
        let mut fcm = MockFcm::new();
        fcm.expect_validate().returning(|| Ok(()));
        let app = crate::build(db.clone(), Arc::new(fcm)).await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { db, url }
    }

    pub fn client(&self) -> PushNotificationsClient {
        PushNotificationsClient::new(self.url.clone()).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionRequest {
    Add(HashSet<TopicId>),
    Replace(HashSet<TopicId>),
}

/// The server's topic subscriptions, and every request that added to or
/// replaced them.
#[derive(Default)]
pub struct RecordingDb {
    subscriptions: Mutex<HashMap<VerifyingKey, HashSet<TopicId>>>,
    requests: Mutex<Vec<SubscriptionRequest>>,
}

impl RecordingDb {
    pub fn topics_of(&self, device: &VerifyingKey) -> HashSet<TopicId> {
        let subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.get(device).cloned().unwrap_or_default()
    }

    pub fn requests(&self) -> Vec<SubscriptionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Driver for RecordingDb {
    async fn store_fcm_token(&self, _: &VerifyingKey, _: &FcmToken) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_fcm_tokens(
        &self,
        _: &[VerifyingKey],
    ) -> anyhow::Result<HashMap<VerifyingKey, FcmToken>> {
        Ok(HashMap::new())
    }

    async fn remove_fcm_token(&self, _: &VerifyingKey) -> anyhow::Result<()> {
        Ok(())
    }

    async fn add_topic_subscriptions(
        &self,
        device: &VerifyingKey,
        topics: &HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions
            .entry(device.clone())
            .or_default()
            .extend(topics.iter().cloned());
        let mut requests = self.requests.lock().unwrap();
        requests.push(SubscriptionRequest::Add(topics.clone()));
        Ok(())
    }

    async fn remove_topic_subscriptions(
        &self,
        device: &VerifyingKey,
        topics: &HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        if let Some(subscribed) = subscriptions.get_mut(device) {
            subscribed.retain(|topic| !topics.contains(topic));
        }
        Ok(())
    }

    async fn get_subscribers_for_topics(
        &self,
        _: &HashSet<TopicId>,
    ) -> anyhow::Result<HashMap<TopicId, Vec<VerifyingKey>>> {
        Ok(HashMap::new())
    }

    async fn update_topic_subscriptions(
        &self,
        device: &VerifyingKey,
        topics: &HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.insert(device.clone(), topics.clone());
        let mut requests = self.requests.lock().unwrap();
        requests.push(SubscriptionRequest::Replace(topics.clone()));
        Ok(())
    }
}
