use anyhow::{Context, Result};
use sqlx::any::AnyPoolOptions;
use sqlx::{AnyPool, Row};

use std::collections::{HashMap, HashSet};

use crate::driver::Driver;
use push_notifications_client::types::{FcmToken, TopicId, VerifyingKey};
use report_common::ReportRow;

pub struct SqlDriver {
    pool: AnyPool,
}

impl SqlDriver {
    pub async fn new(database_url: &str) -> Result<Self> {
        // SQLite only supports one writer at a time; a single connection
        // avoids SQLITE_BUSY errors under concurrent requests.
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .context("failed to connect to database")?;

        // Enable WAL mode for better concurrent read performance.
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await
            .context("failed to enable WAL mode")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS fcm_tokens (
                verifying_key TEXT PRIMARY KEY,
                fcm_token TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .context("failed to create fcm_tokens table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topic_subscribers (
                topic_id TEXT NOT NULL,
                verifying_key TEXT NOT NULL,
                PRIMARY KEY (topic_id, verifying_key)
            )",
        )
        .execute(&pool)
        .await
        .context("failed to create topic_subscribers table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topic_subscribers_pubkey ON topic_subscribers (verifying_key)",
        )
        .execute(&pool)
        .await
        .context("failed to create verifying_key index on topic_subscribers")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reports (
                reported_device_id BLOB NOT NULL,
                reporter_device_id BLOB NOT NULL,
                timestamp BIGINT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .context("failed to create reports table")?;

        Ok(Self { pool })
    }
}

/// Build a batch INSERT query: `INSERT INTO topic_subscribers (topic_id, verifying_key) VALUES ($1, $2), ($3, $4), ... ON CONFLICT DO NOTHING`
/// Returns the query string and a flat list of bind values (topic_id, verifying_key pairs).
fn build_batch_insert(
    verifying_key: &VerifyingKey,
    topic_ids: &HashSet<TopicId>,
) -> (String, Vec<String>) {
    let mut placeholders = Vec::with_capacity(topic_ids.len());
    let mut binds = Vec::with_capacity(topic_ids.len() * 2);
    for (i, topic_id) in topic_ids.iter().enumerate() {
        let p = i * 2;
        placeholders.push(format!("(${}, ${})", p + 1, p + 2));
        binds.push(topic_id.to_string());
        binds.push(verifying_key.to_string());
    }
    let query = format!(
        "INSERT INTO topic_subscribers (topic_id, verifying_key) VALUES {} ON CONFLICT DO NOTHING",
        placeholders.join(", ")
    );
    (query, binds)
}

#[async_trait::async_trait]
impl Driver for SqlDriver {
    async fn store_fcm_token(
        &self,
        verifying_key: &VerifyingKey,
        fcm_token: &FcmToken,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fcm_tokens (verifying_key, fcm_token) VALUES ($1, $2)
             ON CONFLICT (verifying_key) DO UPDATE SET fcm_token = $2",
        )
        .bind(verifying_key.as_str())
        .bind(fcm_token.as_str())
        .execute(&self.pool)
        .await
        .context("failed to store FCM token")?;
        Ok(())
    }

    async fn get_fcm_tokens(
        &self,
        verifying_keys: &[VerifyingKey],
    ) -> Result<HashMap<VerifyingKey, FcmToken>> {
        if verifying_keys.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = verifying_keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();
        let query = format!(
            "SELECT verifying_key, fcm_token FROM fcm_tokens WHERE verifying_key IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query);
        for pk in verifying_keys {
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
                    VerifyingKey::from(r.get::<String, _>("verifying_key")),
                    FcmToken::from(r.get::<String, _>("fcm_token")),
                )
            })
            .collect())
    }

    async fn remove_fcm_token(&self, verifying_key: &VerifyingKey) -> Result<()> {
        sqlx::query("DELETE FROM fcm_tokens WHERE verifying_key = $1")
            .bind(verifying_key.as_str())
            .execute(&self.pool)
            .await
            .context("failed to remove FCM token")?;
        Ok(())
    }

    async fn add_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        if topic_ids.is_empty() {
            return Ok(());
        }
        let (query, binds) = build_batch_insert(verifying_key, topic_ids);
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
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        if topic_ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = topic_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect();
        let query = format!(
            "DELETE FROM topic_subscribers WHERE verifying_key = $1 AND topic_id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query).bind(verifying_key.as_str());
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
    ) -> Result<HashMap<TopicId, Vec<VerifyingKey>>> {
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = topic_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect();
        let query = format!(
            "SELECT topic_id, verifying_key FROM topic_subscribers WHERE topic_id IN ({})",
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

        let mut result: HashMap<TopicId, Vec<VerifyingKey>> = HashMap::new();
        for row in rows {
            let tid = TopicId::from(row.get::<String, _>("topic_id"));
            let pk = VerifyingKey::from(row.get::<String, _>("verifying_key"));
            result.entry(tid).or_default().push(pk);
        }
        Ok(result)
    }

    async fn update_topic_subscriptions(
        &self,
        verifying_key: &VerifyingKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        // Remove all existing subscriptions for this key
        sqlx::query("DELETE FROM topic_subscribers WHERE verifying_key = $1")
            .bind(verifying_key.as_str())
            .execute(&mut *tx)
            .await
            .context("failed to clear subscriptions")?;

        // Batch insert the new set
        if !topic_ids.is_empty() {
            let (insert_query, binds) = build_batch_insert(verifying_key, topic_ids);
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

    async fn store_reports(&self, rows: Vec<ReportRow>) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;
        for row in &rows {
            sqlx::query(
                "INSERT INTO reports (reported_device_id, reporter_device_id, timestamp)
                 VALUES ($1, $2, $3)",
            )
            .bind(row.reported_device_id.to_vec())
            .bind(row.reporter_device_id.to_vec())
            .bind(row.timestamp)
            .execute(&mut *tx)
            .await
            .context("failed to insert report")?;
        }
        tx.commit().await.context("failed to commit transaction")?;
        Ok(())
    }
}
