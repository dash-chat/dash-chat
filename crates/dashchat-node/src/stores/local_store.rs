use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    time::Duration,
};

use chrono::{DateTime, Utc};
use p2panda::Hash;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::{
    compat::Capabilities,
    contact::InboxTopic,
    topic::{AutoRegisteredTopic, kind},
    *,
};

const PRIVATE_KEY_KEY: &str = "private_key";
const AGENT_ID_KEY: &str = "agent_id";

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS identity (
        key TEXT PRIMARY KEY,
        value BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS devices (
        device_id BLOB PRIMARY KEY,
        agent_id BLOB NOT NULL,
        capabilities BLOB NULL
    )",
    "CREATE TABLE IF NOT EXISTS agents (
        agent_id BLOB PRIMARY KEY,
        profile BLOB NULL
    )",
    "CREATE TABLE IF NOT EXISTS subscribed_topics (
        topic_id BLOB PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS active_inboxes (
        topic_id BLOB NOT NULL PRIMARY KEY,
        expires_at_nanos INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS group_chats (
        chat_id BLOB NOT NULL PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS tombstones (
        topic_id BLOB NOT NULL,
        op_hash BLOB NOT NULL,
        PRIMARY KEY (topic_id, op_hash)
    )",
    "CREATE TABLE IF NOT EXISTS unfetched_blob_hashes (
        blob_hash BLOB NOT NULL,
        mailbox_id TEXT NOT NULL,
        PRIMARY KEY (blob_hash, mailbox_id)
    )",
];

#[derive(Clone, Debug)]
pub struct NodeKeys {
    pub private_key: SigningKey,
    pub agent_id: AgentId,
}

impl NodeKeys {
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from(self.private_key.verifying_key())
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

    /// Gracefully close all SQLite connections so the database file handles are released.
    pub async fn close(&self) {
        self.pool.close().await;
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
            let private_key = SigningKey::generate();
            let agent_id = AgentId::from(ActorId::from(SigningKey::generate().verifying_key()));
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
        let rows: Vec<(Topic,)> = sqlx::query_as("SELECT topic_id FROM subscribed_topics")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| *id).collect())
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<Vec<AgentId>> {
        let rows: Vec<(AgentId,)> = sqlx::query_as("SELECT agent_id FROM devices")
            .fetch_all(&self.pool)
            .await?;
        let mut agent_ids: Vec<AgentId> = rows.into_iter().map(|(id,)| id).collect();
        // Deduplicate since multiple devices can map to the same agent
        agent_ids.sort();
        agent_ids.dedup();
        Ok(agent_ids)
    }

    pub async fn lookup_contact_by_device_id(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Option<AgentId>> {
        let row: Option<(AgentId,)> =
            sqlx::query_as("SELECT agent_id FROM devices WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up the device ID for a given agent ID.
    ///
    /// This is temporary, and will not be needed once device gropus are
    /// implemented and [ChatMember] becomes [AgentId].
    pub async fn lookup_contact_by_agent_id(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Option<DeviceId>> {
        let row: Option<(DeviceId,)> =
            sqlx::query_as("SELECT device_id FROM devices WHERE agent_id = ?")
                .bind(agent_id)
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
        device_ids: impl IntoIterator<Item = &DeviceId>,
    ) -> anyhow::Result<HashMap<DeviceId, AgentId>> {
        let device_ids = device_ids.into_iter().collect::<Vec<_>>();
        if device_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(device_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("SELECT device_id, agent_id FROM devices WHERE device_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (DeviceId, AgentId)>(&sql);
        for id in device_ids {
            q = q.bind(*id);
        }
        Ok(q.fetch_all(&self.pool).await?.into_iter().collect())
    }

    pub async fn save_contact(&self, contact: QrCode) -> anyhow::Result<()> {
        self.save_agent_mapping(contact.device_pubkey, contact.agent_id)
            .await
    }

    pub async fn save_agent_mapping(
        &self,
        device_id: DeviceId,
        agent_id: AgentId,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO devices (device_id, agent_id) VALUES (?, ?)")
            .bind(device_id)
            .bind(agent_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("INSERT OR IGNORE INTO agents (agent_id) VALUES (?)")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn save_capabilities(
        &self,
        device_id: DeviceId,
        capabilities: Capabilities,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE devices SET capabilities = ? WHERE device_id = ?")
            .bind(capabilities)
            .bind(device_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn save_profile(&self, agent_id: AgentId, profile: Profile) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO agents (agent_id, profile) VALUES (?, ?)")
            .bind(agent_id)
            .bind(profile)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_capabilities(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Option<Capabilities>> {
        let row: Option<(Option<Capabilities>,)> =
            sqlx::query_as("SELECT capabilities FROM devices WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(capabilities,)| capabilities))
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        let row: Option<(Option<Profile>,)> =
            sqlx::query_as("SELECT profile FROM agents WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(profile,)| profile))
    }

    pub async fn register_topic_as_subscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO subscribed_topics (topic_id) VALUES (?)")
            .bind(topic.to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn register_topic_as_unsubscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM subscribed_topics WHERE topic_id = ?")
            .bind(topic.to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn private_key(&self) -> anyhow::Result<SigningKey> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT value FROM identity WHERE key = ?")
            .bind(PRIVATE_KEY_KEY)
            .fetch_optional(&self.pool)
            .await?;
        let (bytes,) = row.ok_or_else(|| anyhow::anyhow!("Private key field not found"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("identity.private_key is not 32 bytes"))?;
        Ok(SigningKey::from_bytes(&arr))
    }

    pub async fn device_id(&self) -> anyhow::Result<DeviceId> {
        Ok(DeviceId::from(self.private_key().await?.verifying_key()))
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
        let rows: Vec<(Topic, i64)> =
            sqlx::query_as("SELECT topic_id, expires_at_nanos FROM active_inboxes")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(topic, nanos)| InboxTopic {
                expires_at: DateTime::from_timestamp_nanos(nanos),
                topic: topic.upcast::<kind::Inbox>(),
            })
            .collect())
    }

    pub async fn add_active_inbox_topic(&self, inbox_topic: InboxTopic) -> anyhow::Result<()> {
        let nanos = inbox_topic
            .expires_at
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .max(0);
        sqlx::query(
            "INSERT OR REPLACE INTO active_inboxes (topic_id, expires_at_nanos) VALUES (?, ?)",
        )
        .bind(inbox_topic.topic.to_vec())
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

    pub async fn save_group_chat_subscribed(&self, chat_id: ChatId) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO subscribed_topics (topic_id) VALUES (?)")
            .bind(chat_id.to_vec())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO group_chats (chat_id) VALUES (?)")
            .bind(chat_id.to_vec())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_group_chat_ids(&self) -> anyhow::Result<Vec<ChatId>> {
        let rows: Vec<(Topic,)> = sqlx::query_as("SELECT chat_id FROM group_chats")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|(id,)| Topic::<kind::Chat>::from_topic_id(TopicId::from(id)))
            .collect()
    }

    /// Record an operation hash in the per-topic tombstone set. Payloads for
    /// tombstoned operations must never be stored or synced.
    pub async fn add_tombstone(&self, topic: TopicId, op_hash: Hash) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO tombstones (topic_id, op_hash) VALUES (?, ?)")
            .bind(topic.as_bytes().to_vec())
            .bind(op_hash.as_bytes().to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn is_tombstoned(&self, topic: TopicId, op_hash: Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM tombstones WHERE topic_id = ? AND op_hash = ?")
                .bind(topic.as_bytes().to_vec())
                .bind(op_hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn tombstoned_hashes(&self, topic: TopicId) -> anyhow::Result<BTreeSet<Hash>> {
        let rows: Vec<(Vec<u8>,)> =
            sqlx::query_as("SELECT op_hash FROM tombstones WHERE topic_id = ?")
                .bind(topic.as_bytes().to_vec())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(bytes,)| {
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("tombstone op_hash is not 32 bytes"))?;
                Ok(Hash::from_bytes(arr))
            })
            .collect()
    }

    pub async fn add_unfetched_blobs(
        &self,
        mailbox_id: &str,
        hashes: &[iroh_blobs::Hash],
    ) -> anyhow::Result<()> {
        for hash in hashes {
            sqlx::query(
                "INSERT OR IGNORE INTO unfetched_blob_hashes (blob_hash, mailbox_id) VALUES (?, ?)",
            )
            .bind(hash.as_bytes().to_vec())
            .bind(mailbox_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_unfetched_blob(
        &self,
        mailbox_id: &str,
        hash: iroh_blobs::Hash,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM unfetched_blob_hashes WHERE mailbox_id = ? AND blob_hash = ?")
            .bind(mailbox_id)
            .bind(hash.as_bytes().to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_unfetched_blobs(
        &self,
        mailbox_id: &str,
        hashes: &[iroh_blobs::Hash],
    ) -> anyhow::Result<()> {
        for hash in hashes {
            self.remove_unfetched_blob(mailbox_id, *hash).await?;
        }
        Ok(())
    }

    /// Remove every `unfetched_blob_hashes` row for these hashes across ALL
    /// mailboxes. Called when a blob is deleted locally, so the followup task
    /// stops re-announcing a hash this node can no longer serve.
    pub async fn remove_unfetched_blobs_all_mailboxes(
        &self,
        hashes: &[iroh_blobs::Hash],
    ) -> anyhow::Result<()> {
        for hash in hashes {
            sqlx::query("DELETE FROM unfetched_blob_hashes WHERE blob_hash = ?")
                .bind(hash.as_bytes().to_vec())
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn unfetched_blobs_by_mailbox(
        &self,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Vec<iroh_blobs::Hash>>> {
        let rows: Vec<(Vec<u8>, String)> =
            sqlx::query_as("SELECT blob_hash, mailbox_id FROM unfetched_blob_hashes")
                .fetch_all(&self.pool)
                .await?;
        let mut out: std::collections::BTreeMap<String, Vec<iroh_blobs::Hash>> =
            std::collections::BTreeMap::new();
        for (bytes, mailbox_id) in rows {
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("unfetched blob_hash is not 32 bytes"))?;
            out.entry(mailbox_id)
                .or_default()
                .push(iroh_blobs::Hash::from_bytes(arr));
        }
        Ok(out)
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
    async fn test_tombstones_per_topic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_tombstones.db");
        let store = LocalStore::new(&path).await.unwrap();

        let topic_a = *Topic::<kind::Untyped>::new([1; 32]);
        let topic_b = *Topic::<kind::Untyped>::new([2; 32]);
        let hash1 = Hash::digest(b"op1");
        let hash2 = Hash::digest(b"op2");

        assert!(!store.is_tombstoned(topic_a, hash1).await.unwrap());

        store.add_tombstone(topic_a, hash1).await.unwrap();
        store.add_tombstone(topic_a, hash2).await.unwrap();
        store.add_tombstone(topic_b, hash1).await.unwrap();
        // Adding the same hash again is idempotent.
        store.add_tombstone(topic_a, hash1).await.unwrap();

        assert!(store.is_tombstoned(topic_a, hash1).await.unwrap());
        assert!(store.is_tombstoned(topic_a, hash2).await.unwrap());
        // Tombstones are scoped per-topic: hash2 in topic_a does not leak into topic_b.
        assert!(store.is_tombstoned(topic_b, hash1).await.unwrap());
        assert!(!store.is_tombstoned(topic_b, hash2).await.unwrap());

        assert_eq!(
            store.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
        assert_eq!(
            store.tombstoned_hashes(topic_b).await.unwrap(),
            maplit::btreeset![hash1]
        );

        // Tombstones persist across reopening the database.
        drop(store);
        let store = LocalStore::new(&path).await.unwrap();
        assert!(store.is_tombstoned(topic_a, hash1).await.unwrap());
        assert_eq!(
            store.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
    }

    #[tokio::test]
    async fn test_unfetched_blob_hashes_crud() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_unfetched.db");
        let store = LocalStore::new(&path).await.unwrap();

        let mbx_a = "mailbox-a";
        let mbx_b = "mailbox-b";
        let h1 = iroh_blobs::Hash::new([1; 32]);
        let h2 = iroh_blobs::Hash::new([2; 32]);

        store.add_unfetched_blobs(mbx_a, &[h1, h2]).await.unwrap();
        store.add_unfetched_blobs(mbx_b, &[h1]).await.unwrap();
        // Idempotent insert.
        store.add_unfetched_blobs(mbx_a, &[h1]).await.unwrap();

        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get(mbx_a).unwrap().len(), 2);
        assert_eq!(by_mailbox.get(mbx_b).unwrap(), &vec![h1]);

        // Removing h1 from mailbox-a leaves h2 for a, and does not touch mailbox-b.
        store.remove_unfetched_blob(mbx_a, h1).await.unwrap();
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get(mbx_a).unwrap(), &vec![h2]);
        assert_eq!(by_mailbox.get(mbx_b).unwrap(), &vec![h1]);

        // Bulk remove.
        store.remove_unfetched_blobs(mbx_a, &[h2]).await.unwrap();
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert!(by_mailbox.get(mbx_a).is_none());

        // Persists across reopen.
        drop(store);
        let store = LocalStore::new(&path).await.unwrap();
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get(mbx_b).unwrap(), &vec![h1]);
    }

    #[tokio::test]
    async fn test_remove_unfetched_blobs_all_mailboxes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_unfetched_all.db");
        let store = LocalStore::new(&path).await.unwrap();

        let h1 = iroh_blobs::Hash::new([1; 32]);
        let h2 = iroh_blobs::Hash::new([2; 32]);
        store.add_unfetched_blobs("mbx-a", &[h1, h2]).await.unwrap();
        store.add_unfetched_blobs("mbx-b", &[h1]).await.unwrap();

        // Removing h1 across all mailboxes clears it from both mbx-a and mbx-b,
        // but leaves h2 (still needed by mbx-a).
        store
            .remove_unfetched_blobs_all_mailboxes(&[h1])
            .await
            .unwrap();

        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get("mbx-a").unwrap(), &vec![h2]);
        assert!(by_mailbox.get("mbx-b").is_none());
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
