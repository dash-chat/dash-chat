use anyhow::{Context, Result};
use sqlx::{AnyPool, Row};

use std::collections::HashSet;

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

        Ok(Self { pool })
    }
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

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<FcmToken>> {
        let row = sqlx::query("SELECT fcm_token FROM fcm_tokens WHERE public_key = $1")
            .bind(public_key.as_str())
            .fetch_optional(&self.pool)
            .await
            .context("failed to get FCM token")?;
        Ok(row.map(|r| FcmToken::from(r.get::<String, _>("fcm_token"))))
    }

    async fn subscribe_to_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.context("failed to begin transaction")?;
        for topic_id in topic_ids {
            sqlx::query(
                "INSERT INTO topic_subscribers (topic_id, public_key) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(topic_id.as_str())
            .bind(public_key.as_str())
            .execute(&mut *tx)
            .await
            .context("failed to subscribe to topic")?;
        }
        tx.commit().await.context("failed to commit transaction")?;
        Ok(())
    }

    async fn unsubscribe_from_topics(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.context("failed to begin transaction")?;
        for topic_id in topic_ids {
            sqlx::query(
                "DELETE FROM topic_subscribers WHERE topic_id = $1 AND public_key = $2",
            )
            .bind(topic_id.as_str())
            .bind(public_key.as_str())
            .execute(&mut *tx)
            .await
            .context("failed to unsubscribe from topic")?;
        }
        tx.commit().await.context("failed to commit transaction")?;
        Ok(())
    }

    async fn get_subscribers(&self, topic_id: &TopicId) -> Result<Vec<PublicKey>> {
        let rows =
            sqlx::query("SELECT public_key FROM topic_subscribers WHERE topic_id = $1")
                .bind(topic_id.as_str())
                .fetch_all(&self.pool)
                .await
                .context("failed to get subscribers")?;
        Ok(rows
            .into_iter()
            .map(|r| PublicKey::from(r.get::<String, _>("public_key")))
            .collect())
    }

    async fn set_subscriptions(
        &self,
        public_key: &PublicKey,
        topic_ids: &HashSet<TopicId>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.context("failed to begin transaction")?;

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
        }

        // Insert new subscriptions
        for topic_id in topic_ids {
            sqlx::query(
                "INSERT INTO topic_subscribers (topic_id, public_key) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(topic_id.as_str())
            .bind(public_key.as_str())
            .execute(&mut *tx)
            .await
            .context("failed to insert subscription")?;
        }

        tx.commit().await.context("failed to commit transaction")?;
        Ok(())
    }
}
