pub mod mem;
pub mod sql;

use std::collections::HashSet;

use push_notifications_client::types::{FcmToken, PublicKey, TopicId};

#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    async fn store_fcm_token(
        &self,
        public_key: &PublicKey,
        fcm_token: &FcmToken,
    ) -> anyhow::Result<()>;

    async fn get_fcm_token(&self, public_key: &PublicKey) -> anyhow::Result<Option<FcmToken>>;

    async fn remove_fcm_token(&self, public_key: &PublicKey) -> anyhow::Result<()>;

    async fn subscribe_to_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;

    async fn unsubscribe_from_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;

    async fn get_subscribers(&self, topic_id: &TopicId) -> anyhow::Result<Vec<PublicKey>>;

    /// Replace all subscriptions for a public key with the given set.
    /// Removes any existing subscriptions not in `topic_ids`.
    async fn set_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;
}
