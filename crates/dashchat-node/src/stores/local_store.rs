use std::{
    collections::{BTreeSet, HashMap},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use p2panda_auth::Access;
use p2panda_core::{Hash, Operation, PublicKey};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio::sync::Mutex;

use crate::{
    contact::InboxTopic,
    topic::{AutoRegisteredTopic, TopicId},
    *,
};

const PRIVATE_KEY_KEY: &str = "private_key";
const AGENT_ID_KEY: &str = "agent_id";

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS identity (
        key TEXT PRIMARY KEY,
        value BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS contacts (
        device_id BLOB PRIMARY KEY,
        agent_id BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS subscribed_topics (
        topic_id BLOB PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS active_inboxes (
        topic_id BLOB NOT NULL PRIMARY KEY,
        expires_at_nanos INTEGER NOT NULL
    )",
];

#[derive(Clone, Debug)]
pub struct NodeKeys {
    pub private_key: PrivateKey,
    pub agent_id: AgentId,
}

impl NodeKeys {
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from(self.private_key.public_key())
    }
}

#[derive(Clone)]
pub struct LocalStore {
    pool: SqlitePool,
}

impl LocalStore {
    pub async fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(30));
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        for sql in MIGRATIONS {
            sqlx::query(sql).execute(&pool).await?;
        }

        let store = Self { pool };
        store.ensure_initialized().await?;
        Ok(store)
    }

    /// If the database is not initialized, initialize with random keys
    async fn ensure_initialized(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let existing: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT value FROM identity WHERE key = ?")
                .bind(PRIVATE_KEY_KEY)
                .fetch_optional(&mut *tx)
                .await?;
        if existing.is_none() {
            let private_key = PrivateKey::new();
            let agent_id = AgentId::from(ActorId::from(PrivateKey::new().public_key()));
            sqlx::query("INSERT INTO identity (key, value) VALUES (?, ?)")
                .bind(PRIVATE_KEY_KEY)
                .bind(private_key.as_bytes().to_vec())
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO identity (key, value) VALUES (?, ?)")
                .bind(AGENT_ID_KEY)
                .bind(agent_id.as_bytes().to_vec())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn node_keys(&self) -> anyhow::Result<NodeKeys> {
        Ok(NodeKeys {
            private_key: self.private_key().await?,
            agent_id: self.agent_id().await?,
        })
    }

    pub async fn subscribed_topics(&self) -> anyhow::Result<BTreeSet<TopicId>> {
        let rows: Vec<(TopicId,)> = sqlx::query_as("SELECT topic_id FROM subscribed_topics")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<Vec<AgentId>> {
        let rows: Vec<(AgentId,)> = sqlx::query_as("SELECT agent_id FROM contacts")
            .fetch_all(&self.pool)
            .await?;
        let mut agent_ids: Vec<AgentId> = rows.into_iter().map(|(id,)| id).collect();
        // Deduplicate since multiple devices can map to the same agent
        agent_ids.sort();
        agent_ids.dedup();
        Ok(agent_ids)
    }

    pub async fn lookup_contact(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        let row: Option<(AgentId,)> =
            sqlx::query_as("SELECT agent_id FROM contacts WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up multiple contacts in a single query.
    ///
    /// Returns a map from `DeviceId` to its `AgentId`. Devices that have no
    /// contact entry are simply absent from the map — the caller can compare
    /// against the input slice to find which lookups missed.
    pub async fn lookup_contacts(
        &self,
        device_ids: &[DeviceId],
    ) -> anyhow::Result<HashMap<DeviceId, AgentId>> {
        if device_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(device_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("SELECT device_id, agent_id FROM contacts WHERE device_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (DeviceId, AgentId)>(&sql);
        for id in device_ids {
            q = q.bind(*id);
        }
        Ok(q.fetch_all(&self.pool).await?.into_iter().collect())
    }

    pub async fn save_contact(&self, contact: QrCode) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO contacts (device_id, agent_id) VALUES (?, ?)")
            .bind(contact.device_pubkey)
            .bind(contact.agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn register_topic_as_subscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO subscribed_topics (topic_id) VALUES (?)")
            .bind(*topic)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn register_topic_as_unsubscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM subscribed_topics WHERE topic_id = ?")
            .bind(*topic)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn private_key(&self) -> anyhow::Result<PrivateKey> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT value FROM identity WHERE key = ?")
            .bind(PRIVATE_KEY_KEY)
            .fetch_optional(&self.pool)
            .await?;
        let (bytes,) = row.ok_or_else(|| anyhow::anyhow!("Private key field not found"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("identity.private_key is not 32 bytes"))?;
        Ok(PrivateKey::from_bytes(&arr))
    }

    pub async fn device_id(&self) -> anyhow::Result<DeviceId> {
        Ok(DeviceId::from(self.private_key().await?.public_key()))
    }

    pub async fn agent_id(&self) -> anyhow::Result<AgentId> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT value FROM identity WHERE key = ?")
            .bind(AGENT_ID_KEY)
            .fetch_optional(&self.pool)
            .await?;
        let (bytes,) = row.ok_or_else(|| anyhow::anyhow!("Agent ID field not found"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("identity.agent_id is not 32 bytes"))?;
        Ok(AgentId::from(crate::ActorId::from_bytes(&arr)?))
    }

    pub async fn get_active_inbox_topics(&self) -> anyhow::Result<BTreeSet<InboxTopic>> {
        let rows: Vec<(TopicId, i64)> =
            sqlx::query_as("SELECT topic_id, expires_at_nanos FROM active_inboxes")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(topic_id, nanos)| InboxTopic {
                expires_at: DateTime::from_timestamp_nanos(nanos),
                topic: Topic::new(*topic_id),
            })
            .collect())
    }

    pub async fn add_active_inbox_topic(&self, topic: InboxTopic) -> anyhow::Result<()> {
        let nanos = topic.expires_at.timestamp_nanos_opt().unwrap_or(0).max(0);
        sqlx::query(
            "INSERT OR REPLACE INTO active_inboxes (topic_id, expires_at_nanos) VALUES (?, ?)",
        )
        .bind(*topic.topic)
        .bind(nanos)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn prune_expired_active_inbox_topics(
        &self,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let nanos = expires_at.timestamp_nanos_opt().unwrap_or(0).max(0);
        sqlx::query("DELETE FROM active_inboxes WHERE expires_at_nanos < ?")
            .bind(nanos)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::topic::Topic;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_initialize_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_initialize_random.db");
        let store = LocalStore::new(&path).await.unwrap();
        let private_key = store.private_key().await.unwrap();
        let agent_id = store.agent_id().await.unwrap();
        store.ensure_initialized().await.unwrap();
        assert_eq!(
            store.private_key().await.unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().await.unwrap(), agent_id);

        drop(store);

        let store = LocalStore::new(path).await.unwrap();
        assert_eq!(
            store.private_key().await.unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().await.unwrap(), agent_id);
    }

    #[tokio::test]
    async fn test_prune_expired_active_inbox_topics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_prune_inbox_topics.db");
        let store = LocalStore::new(&path).await.unwrap();

        let now = Utc::now();
        let expired = now - Duration::days(1);
        let valid = now + Duration::days(1);
        let more_valid = now + Duration::days(10);

        let mut topics = maplit::btreeset![
            InboxTopic {
                expires_at: expired,
                topic: Topic::new([1; 32]),
            },
            InboxTopic {
                expires_at: valid,
                topic: Topic::new([2; 32]),
            },
            InboxTopic {
                expires_at: more_valid,
                topic: Topic::new([3; 32]),
            },
        ];

        for t in &topics {
            store.add_active_inbox_topic(t.clone()).await.unwrap();
        }

        let loaded_topics = store.get_active_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);

        store.prune_expired_active_inbox_topics(now).await.unwrap();
        topics.pop_first().unwrap();

        let loaded_topics = store.get_active_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);

        store
            .prune_expired_active_inbox_topics(more_valid)
            .await
            .unwrap();
        topics.pop_first().unwrap();

        let loaded_topics = store.get_active_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);
    }
}
