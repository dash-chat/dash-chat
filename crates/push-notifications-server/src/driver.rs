pub mod mem;
pub mod sql;

use std::collections::{HashMap, HashSet};

use push_notifications_client::types::{FcmToken, TopicId, VerifyingKey};
use report_common::ReportRow;

#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    async fn store_fcm_token(
        &self,
        verifying_key: &VerifyingKey,
        fcm_token: &FcmToken,
    ) -> anyhow::Result<()>;

    async fn get_fcm_tokens(
        &self,
        verifying_keys: &[VerifyingKey],
    ) -> anyhow::Result<HashMap<VerifyingKey, FcmToken>>;

    async fn remove_fcm_token(&self, verifying_key: &VerifyingKey) -> anyhow::Result<()>;

    async fn add_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;

    async fn remove_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;

    async fn get_subscribers_for_topics(
        &self,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<HashMap<TopicId, Vec<VerifyingKey>>>;

    /// Replace all subscriptions for a public key with the given set.
    /// Removes any existing subscriptions not in `topic_ids`.
    async fn update_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> anyhow::Result<()>;

    /// Persist one row per reported device (the reporter and timestamp are
    /// duplicated across the rows produced from a single report request).
    async fn store_reports(&self, rows: Vec<ReportRow>) -> anyhow::Result<()>;
}
