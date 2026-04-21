use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

use anyhow::Result;

use crate::{
    driver::Driver,
    types::{FcmToken, PublicKey, TopicId},
};

pub struct MemDb {
    tokens: Mutex<HashMap<PublicKey, FcmToken>>,
    subscriptions: Mutex<HashMap<TopicId, HashSet<PublicKey>>>,
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
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &FcmToken) -> Result<()> {
        self.tokens
            .lock()
            .await
            .insert(public_key.clone(), fcm_token.clone());
        Ok(())
    }

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<FcmToken>> {
        Ok(self.tokens.lock().await.get(public_key).cloned())
    }

    async fn subscribe_to_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;
        for topic_id in topic_ids {
            subs.entry(topic_id.clone())
                .or_default()
                .insert(public_key.clone());
        }
        Ok(())
    }

    async fn unsubscribe_from_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;
        for topic_id in topic_ids {
            if let Some(subscribers) = subs.get_mut(topic_id) {
                subscribers.remove(public_key);
                if subscribers.is_empty() {
                    subs.remove(topic_id);
                }
            }
        }
        Ok(())
    }

    async fn get_subscribers(&self, topic_id: &TopicId) -> Result<Vec<PublicKey>> {
        let subs = self.subscriptions.lock().await;
        Ok(subs
            .get(topic_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default())
    }

    async fn set_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let new_topics: HashSet<TopicId> = topic_ids.iter().cloned().collect();
        let mut subs = self.subscriptions.lock().await;

        // Remove public_key from topics not in new set
        subs.retain(|topic_id, subscribers| {
            if !new_topics.contains(topic_id) {
                subscribers.remove(public_key);
                !subscribers.is_empty()
            } else {
                true
            }
        });

        // Add public_key to all new topics
        for topic_id in topic_ids {
            subs.entry(topic_id.clone())
                .or_default()
                .insert(public_key.clone());
        }

        Ok(())
    }
}
