use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

use anyhow::Result;

use crate::driver::Driver;
use push_notifications_client::types::{FcmToken, PublicKey, TopicId};

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

    async fn get_fcm_tokens(&self, public_keys: &[PublicKey]) -> Result<HashMap<PublicKey, FcmToken>> {
        let tokens = self.tokens.lock().await;
        Ok(public_keys
            .iter()
            .filter_map(|pk| tokens.get(pk).map(|t| (pk.clone(), t.clone())))
            .collect())
    }

    async fn remove_fcm_token(&self, public_key: &PublicKey) -> Result<()> {
        self.tokens.lock().await.remove(public_key);
        Ok(())
    }

    async fn subscribe_to_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
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
        topic_ids: &HashSet<TopicId>,
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

    async fn get_subscribers_for_topics(
        &self,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<HashMap<TopicId, Vec<PublicKey>>> {
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

    async fn set_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;

        // Remove public_key from topics not in new set
        subs.retain(|topic_id, subscribers| {
            if !topic_ids.contains(topic_id) {
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
