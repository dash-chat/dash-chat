use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

use anyhow::Result;

use crate::driver::Driver;
use push_notifications_client::types::{FcmToken, VerifyingKey, TopicId};

pub struct MemDb {
    tokens: Mutex<HashMap<VerifyingKey, FcmToken>>,
    subscriptions: Mutex<HashMap<TopicId, HashSet<VerifyingKey>>>,
}

impl MemDb {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl Driver for MemDb {
    async fn store_fcm_token(&self, verifying_key: &VerifyingKey, fcm_token: &FcmToken) -> Result<()> {
        self.tokens
            .lock()
            .await
            .insert(verifying_key.clone(), fcm_token.clone());
        Ok(())
    }

    async fn get_fcm_tokens(
        &self,
        verifying_keys: &[VerifyingKey],
    ) -> Result<HashMap<VerifyingKey, FcmToken>> {
        let tokens = self.tokens.lock().await;
        Ok(verifying_keys
            .iter()
            .filter_map(|pk| tokens.get(pk).map(|t| (pk.clone(), t.clone())))
            .collect())
    }

    async fn remove_fcm_token(&self, verifying_key: &VerifyingKey) -> Result<()> {
        self.tokens.lock().await.remove(verifying_key);
        Ok(())
    }

    async fn add_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;
        for topic_id in topic_ids {
            subs.entry(topic_id.clone())
                .or_default()
                .insert(verifying_key.clone());
        }
        Ok(())
    }

    async fn remove_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;
        for topic_id in topic_ids {
            if let Some(subscribers) = subs.get_mut(topic_id) {
                subscribers.remove(verifying_key);
                if subscribers.is_empty() {
                    subs.remove(topic_id);
                }
            }
        }
        Ok(())
    }

    async fn get_subscribers_for_topics(
        &self,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<HashMap<TopicId, Vec<VerifyingKey>>> {
        let subs = self.subscriptions.lock().await;
        Ok(topic_ids
            .iter()
            .map(|tid| {
                let subscribers = subs
                    .get(tid)
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default();
                (tid.clone(), subscribers)
            })
            .collect())
    }

    async fn update_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;

        // Remove verifying_key from topics not in new set
        subs.retain(|topic_id, subscribers| {
            if !topic_ids.contains(topic_id) {
                subscribers.remove(verifying_key);
                !subscribers.is_empty()
            } else {
                true
            }
        });

        // Add verifying_key to all new topics
        for topic_id in topic_ids {
            subs.entry(topic_id.clone())
                .or_default()
                .insert(verifying_key.clone());
        }

        Ok(())
    }
}
