use anyhow::{Context, Result};
use sqlx::{AnyPool, Row};

use std::collections::{HashMap, HashSet};

use crate::driver::Driver;
use push_notifications_client::types::{FcmToken, PublicKey, TopicId};

pub struct SqlDriver {
    pool: AnyPool,
}

impl SqlDriver {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = AnyPool::connect(database_url)
            .await
            .context("failed to connect to database")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS fcm_tokens (
                public_key TEXT PRIMARY KEY,
                fcm_token TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .context("failed to create fcm_tokens table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topic_subscribers (
                topic_id TEXT NOT NULL,
                public_key TEXT NOT NULL,
                PRIMARY KEY (topic_id, public_key)
            )",
        )
        .execute(&pool)
        .await
        .context("failed to create topic_subscribers table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topic_subscribers_pubkey ON topic_subscribers (public_key)",
        )
        .execute(&pool)
        .await
        .context("failed to create public_key index on topic_subscribers")?;

        Ok(Self { pool })
    }
}

/// Build a batch INSERT query: `INSERT INTO topic_subscribers (topic_id, public_key) VALUES ($1, $2), ($3, $4), ... ON CONFLICT DO NOTHING`
/// Returns the query string and a flat list of bind values (topic_id, public_key pairs).
fn build_batch_insert(
    public_key: &PublicKey,
    topic_ids: &HashSet<TopicId>,
) -> (String, Vec<String>) {
    let mut placeholders = Vec::with_capacity(topic_ids.len());
    let mut binds = Vec::with_capacity(topic_ids.len() * 2);
    for (i, topic_id) in topic_ids.iter().enumerate() {
        let p = i * 2;
        placeholders.push(format!("(${}, ${})", p + 1, p + 2));
        binds.push(topic_id.to_string());
        binds.push(public_key.to_string());
    }
    let query = format!(
        "INSERT INTO topic_subscribers (topic_id, public_key) VALUES {} ON CONFLICT DO NOTHING",
        placeholders.join(", ")
    );
    (query, binds)
}

#[async_trait::async_trait]
impl Driver for SqlDriver {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &FcmToken) -> Result<()> {
        sqlx::query(
            "INSERT INTO fcm_tokens (public_key, fcm_token) VALUES ($1, $2)
             ON CONFLICT (public_key) DO UPDATE SET fcm_token = $2",
        )
        .bind(public_key.as_str())
        .bind(fcm_token.as_str())
        .execute(&self.pool)
        .await
        .context("failed to store FCM token")?;
        Ok(())
    }

    async fn get_fcm_tokens(
        &self,
        public_keys: &[PublicKey],
    ) -> Result<HashMap<PublicKey, FcmToken>> {
        if public_keys.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = public_keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();
        let query = format!(
            "SELECT public_key, fcm_token FROM fcm_tokens WHERE public_key IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query);
        for pk in public_keys {
            q = q.bind(pk.as_str());
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .context("failed to get FCM tokens")?;

        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    PublicKey::from(r.get::<String, _>("public_key")),
                    FcmToken::from(r.get::<String, _>("fcm_token")),
                )
            })
            .collect())
    }

    async fn remove_fcm_token(&self, public_key: &PublicKey) -> Result<()> {
        sqlx::query("DELETE FROM fcm_tokens WHERE public_key = $1")
            .bind(public_key.as_str())
            .execute(&self.pool)
            .await
            .context("failed to remove FCM token")?;
        Ok(())
    }

    async fn add_topic_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        if topic_ids.is_empty() {
            return Ok(());
        }
        let (query, binds) = build_batch_insert(public_key, topic_ids);
        let mut q = sqlx::query(&query);
        for val in &binds {
            q = q.bind(val);
        }
        q.execute(&self.pool)
            .await
            .context("failed to subscribe to topics")?;
        Ok(())
    }

    async fn remove_topic_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        if topic_ids.is_empty() {
            return Ok(());
        }
        // DELETE ... WHERE public_key = $1 AND topic_id IN ($2, $3, ...)
        let placeholders: Vec<String> = topic_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect();
        let query = format!(
            "DELETE FROM topic_subscribers WHERE public_key = $1 AND topic_id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query).bind(public_key.as_str());
        for tid in topic_ids {
            q = q.bind(tid.as_str());
        }
        q.execute(&self.pool)
            .await
            .context("failed to unsubscribe from topics")?;
        Ok(())
    }

    async fn get_subscribers_for_topics(
        &self,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<HashMap<TopicId, Vec<PublicKey>>> {
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = topic_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();
        let query = format!(
            "SELECT topic_id, public_key FROM topic_subscribers WHERE topic_id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query);
        for tid in topic_ids {
            q = q.bind(tid.as_str());
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .context("failed to get subscribers")?;

        let mut result: HashMap<TopicId, Vec<PublicKey>> = HashMap::new();
        for row in rows {
            let tid = TopicId::from(row.get::<String, _>("topic_id"));
            let pk = PublicKey::from(row.get::<String, _>("public_key"));
            result.entry(tid).or_default().push(pk);
        }
        Ok(result)
    }

    async fn update_topic_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        // Remove subscriptions not in the new set
        if topic_ids.is_empty() {
            sqlx::query("DELETE FROM topic_subscribers WHERE public_key = $1")
                .bind(public_key.as_str())
                .execute(&mut *tx)
                .await
                .context("failed to clear subscriptions")?;
        } else {
            // sqlx Any doesn't support array binds, so build placeholders dynamically
            let placeholders: Vec<String> = topic_ids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("${}", i + 2))
                .collect();
            let query = format!(
                "DELETE FROM topic_subscribers WHERE public_key = $1 AND topic_id NOT IN ({})",
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&query).bind(public_key.as_str());
            for topic_id in topic_ids {
                q = q.bind(topic_id.to_string());
            }
            q.execute(&mut *tx)
                .await
                .context("failed to remove old subscriptions")?;

            // Batch insert new subscriptions
            let (insert_query, binds) = build_batch_insert(public_key, topic_ids);
            let mut q = sqlx::query(&insert_query);
            for val in &binds {
                q = q.bind(val);
            }
            q.execute(&mut *tx)
                .await
                .context("failed to insert subscriptions")?;
        }

        tx.commit().await.context("failed to commit transaction")?;
        Ok(())
    }
}
