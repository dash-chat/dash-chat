use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;

use crate::{
    driver::Driver,
    types::{FcmToken, PublicKey, TopicId},
};

const FCM_TOKENS: TableDefinition<&str, &str> = TableDefinition::new("fcm_tokens");
/// Keys are "topic_id\0public_key", value is empty (presence-based).
const TOPIC_SUBSCRIBERS: TableDefinition<&str, &str> = TableDefinition::new("topic_subscribers");

pub struct RedbDriver {
    db: Arc<Database>,
}

impl RedbDriver {
    pub fn new(path: &Path) -> Result<Self> {
        // Create parent directory if it does not exist
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(path).context("failed to open redb database")?;

        // Ensure tables exist.
        let txn = db.begin_write()?;
        txn.open_table(FCM_TOKENS)?;
        txn.open_table(TOPIC_SUBSCRIBERS)?;
        txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }
}

#[async_trait::async_trait]
impl Driver for RedbDriver {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &FcmToken) -> Result<()> {
        let db = self.db.clone();
        let pk = public_key.to_string();
        let token = fcm_token.to_string();

        // redb operations are synchronous and can block on disk I/O,
        // so we run them off the tokio runtime to avoid stalling other tasks.
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(FCM_TOKENS)?;
                table.insert(pk.as_str(), token.as_str())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .context("blocking task panicked")?
    }

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<FcmToken>> {
        let db = self.db.clone();
        let pk = public_key.to_string();

        tokio::task::spawn_blocking(move || -> Result<Option<FcmToken>> {
            let txn = db.begin_read()?;
            let table = txn.open_table(FCM_TOKENS)?;
            let value = table.get(pk.as_str())?;
            Ok(value.map(|v| FcmToken::from(v.value().to_string())))
        })
        .await
        .context("blocking task panicked")?
    }

    async fn subscribe_to_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let db = self.db.clone();
        let pk = public_key.to_string();
        let topics: Vec<String> = topic_ids.iter().map(|t| t.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(TOPIC_SUBSCRIBERS)?;
                for topic in &topics {
                    let key = format!("{topic}\0{pk}");
                    table.insert(key.as_str(), "")?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .context("blocking task panicked")?
    }

    async fn unsubscribe_from_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let db = self.db.clone();
        let pk = public_key.to_string();
        let topics: Vec<String> = topic_ids.iter().map(|t| t.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(TOPIC_SUBSCRIBERS)?;
                for topic in &topics {
                    let key = format!("{topic}\0{pk}");
                    table.remove(key.as_str())?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .context("blocking task panicked")?
    }

    async fn get_subscribers(&self, topic_id: &TopicId) -> Result<Vec<PublicKey>> {
        let db = self.db.clone();
        let topic = topic_id.to_string();

        tokio::task::spawn_blocking(move || -> Result<Vec<PublicKey>> {
            let txn = db.begin_read()?;
            let table = txn.open_table(TOPIC_SUBSCRIBERS)?;

            let prefix = format!("{topic}\0");
            let range_start = prefix.as_str();
            // Create an exclusive upper bound by incrementing the last byte of the prefix
            let mut range_end = prefix.clone();
            // Replace trailing \0 with \x01 to get the next prefix
            range_end.pop();
            range_end.push('\x01');

            let mut subscribers = Vec::new();
            for entry in table.range(range_start..range_end.as_str())? {
                let (key, _) = entry?;
                let key_str = key.value();
                // Key format: "topic_id\0public_key"
                if let Some(pk) = key_str.strip_prefix(&prefix) {
                    subscribers.push(PublicKey::from(pk.to_string()));
                }
            }
            Ok(subscribers)
        })
        .await
        .context("blocking task panicked")?
    }

    async fn set_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &[TopicId],
    ) -> Result<()> {
        let db = self.db.clone();
        let pk = public_key.to_string();
        let new_topics: std::collections::HashSet<String> =
            topic_ids.iter().map(|t| t.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(TOPIC_SUBSCRIBERS)?;

                // Find and remove existing subscriptions for this public key
                // that are not in the new set.
                // We scan the whole table looking for entries ending in \0{pk}.
                let suffix = format!("\0{pk}");
                let mut to_remove = Vec::new();
                for entry in table.iter()? {
                    let (key, _) = entry?;
                    let key_str = key.value();
                    if key_str.ends_with(&suffix) {
                        let topic = &key_str[..key_str.len() - suffix.len()];
                        if !new_topics.contains(topic) {
                            to_remove.push(key_str.to_string());
                        }
                    }
                }
                for key in &to_remove {
                    table.remove(key.as_str())?;
                }

                // Insert all new subscriptions (idempotent)
                for topic in &new_topics {
                    let key = format!("{topic}\0{pk}");
                    table.insert(key.as_str(), "")?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .context("blocking task panicked")?
    }
}
