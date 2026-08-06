use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::{
    contact::InboxTopic,
    topic::{AutoRegisteredTopic, kind},
    *,
};

const PRIVATE_KEY_KEY: &str = "private_key";
const AGENT_ID_KEY: &str = "agent_id";

/// Distinguishes the two roles an inbox topic can play for this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[repr(i64)]
enum InboxRole {
    /// An inbox we advertise in our QR code and receive contact requests on.
    Advertised = 0,
    /// A private inbox we minted while scanning someone's QR, used only to
    /// receive their `ContactRequestAck`.
    Reply = 1,
}

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS identity (
        key TEXT PRIMARY KEY,
        value BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS subscribed_topics (
        topic_id BLOB PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS active_inboxes (
        topic_id BLOB NOT NULL PRIMARY KEY,
        expires_at_nanos INTEGER NOT NULL,
        role INTEGER NOT NULL DEFAULT 0,
        expected_ack_author BLOB NULL
    )",
    "CREATE TABLE IF NOT EXISTS unfetched_blob_hashes (
        blob_hash BLOB NOT NULL,
        mailbox_id TEXT NOT NULL,
        PRIMARY KEY (blob_hash, mailbox_id)
    )",
    "CREATE TABLE IF NOT EXISTS reported_contacts (
        device_id BLOB NOT NULL,
        mailbox_id TEXT NOT NULL,
        PRIMARY KEY (device_id, mailbox_id)
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

/// The [`LocalStore`] stores only information that is specific to the operation of this device.
/// It is completely independent and orthogonal to the [`crate::stores::OpStore`] and [`crate::stores::OpProjection`].
/// If data doesn't belong in a log to be synced with other devices (including in one's own device group),
/// it probably belongs here.
#[derive(Clone)]
pub struct LocalStore {
    pool: SqlitePool,
}

impl LocalStore {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
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
        let rows: Vec<(Topic<kind::Untyped>,)> =
            sqlx::query_as("SELECT topic_id FROM subscribed_topics")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| *id).collect())
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

    /// Inbox topics this node created and advertises via its QR code.
    pub async fn get_advertised_inbox_topics(&self) -> anyhow::Result<BTreeSet<InboxTopic>> {
        self.get_inbox_topics(InboxRole::Advertised).await
    }

    /// Reply inbox topics this node created for a specific contact exchange and
    /// is awaiting a `ContactRequestAck` on. Kept separate from advertised
    /// inboxes so the frontend's contact-request scan only looks at inboxes
    /// meant to receive requests, not our own private reply channels.
    pub async fn get_reply_inbox_topics(&self) -> anyhow::Result<BTreeSet<InboxTopic>> {
        self.get_inbox_topics(InboxRole::Reply).await
    }

    pub async fn get_reply_inbox_topics_with_author(
        &self,
    ) -> anyhow::Result<Vec<(InboxTopic, DeviceId)>> {
        let rows: Vec<(Topic<kind::Untyped>, i64, DeviceId)> = sqlx::query_as(
            "SELECT topic_id, expires_at_nanos, expected_ack_author FROM active_inboxes WHERE role = ?",
        )
        .bind(InboxRole::Reply)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(topic, nanos, author)| {
                (
                    InboxTopic {
                        expires_at: DateTime::from_timestamp_nanos(nanos),
                        topic: topic.upcast::<kind::Inbox>(),
                    },
                    author,
                )
            })
            .collect())
    }

    async fn get_inbox_topics(&self, role: InboxRole) -> anyhow::Result<BTreeSet<InboxTopic>> {
        let rows: Vec<(Topic<kind::Untyped>, i64)> =
            sqlx::query_as("SELECT topic_id, expires_at_nanos FROM active_inboxes WHERE role = ?")
                .bind(role)
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
        self.add_inbox_topic(inbox_topic, InboxRole::Advertised)
            .await
    }

    pub async fn add_reply_inbox_topic(
        &self,
        inbox_topic: InboxTopic,
        expected_ack_author: DeviceId,
    ) -> anyhow::Result<()> {
        let nanos = inbox_topic
            .expires_at
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .max(0);
        sqlx::query(
            "INSERT OR REPLACE INTO active_inboxes (topic_id, expires_at_nanos, role, expected_ack_author) VALUES (?, ?, ?, ?)",
        )
        .bind(inbox_topic.topic.to_vec())
        .bind(nanos)
        .bind(InboxRole::Reply)
        .bind(expected_ack_author)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn has_pending_reply_inbox_for(&self, device_id: DeviceId) -> anyhow::Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM active_inboxes WHERE expected_ack_author = ? AND role = ?",
        )
        .bind(device_id)
        .bind(InboxRole::Reply)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn get_reply_inbox_expected_ack_author(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<Option<DeviceId>> {
        let row: Option<(DeviceId,)> = sqlx::query_as(
            "SELECT expected_ack_author FROM active_inboxes WHERE topic_id = ? AND role = ?",
        )
        .bind(topic.as_bytes().to_vec())
        .bind(InboxRole::Reply)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    async fn add_inbox_topic(
        &self,
        inbox_topic: InboxTopic,
        role: InboxRole,
    ) -> anyhow::Result<()> {
        let nanos = inbox_topic
            .expires_at
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .max(0);
        sqlx::query(
            "INSERT OR REPLACE INTO active_inboxes (topic_id, expires_at_nanos, role) VALUES (?, ?, ?)",
        )
        .bind(inbox_topic.topic.to_vec())
        .bind(nanos)
        .bind(role)
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

    /// Record that each of `device_ids` has been successfully reported to each
    /// of `mailbox_ids`. The `(device_id, mailbox_id)` table is denormalized so
    /// a later report can skip mailboxes that already have every reported id.
    pub async fn record_reported_devices(
        &self,
        device_ids: &[DeviceId],
        mailbox_ids: &[String],
    ) -> anyhow::Result<()> {
        for device_id in device_ids {
            for mailbox_id in mailbox_ids {
                sqlx::query(
                    "INSERT OR IGNORE INTO reported_contacts (device_id, mailbox_id) VALUES (?, ?)",
                )
                .bind(*device_id)
                .bind(mailbox_id)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Mailboxes that have already received a report covering every one of
    /// `device_ids` (the intersection of each device's reported-mailbox set).
    /// A report covering `device_ids` can skip exactly these mailboxes, since
    /// any mailbox missing even one id still needs the full request. Returns an
    /// empty set when `device_ids` is empty.
    pub async fn mailboxes_reported_to_all(
        &self,
        device_ids: &[DeviceId],
    ) -> anyhow::Result<BTreeSet<String>> {
        let mut common: Option<BTreeSet<String>> = None;
        for device_id in device_ids {
            let reported = self.reported_mailboxes_for_device(*device_id).await?;
            common = Some(match common {
                None => reported,
                Some(prev) => prev.intersection(&reported).cloned().collect(),
            });
            if common.as_ref().is_some_and(|c| c.is_empty()) {
                break;
            }
        }
        Ok(common.unwrap_or_default())
    }

    /// Every mailbox that has received a report covering at least one of
    /// `device_ids` (the union of each device's reported-mailbox set). Returns
    /// an empty set when `device_ids` is empty.
    pub async fn mailboxes_reported_to_any(
        &self,
        device_ids: &[DeviceId],
    ) -> anyhow::Result<BTreeSet<String>> {
        let mut union = BTreeSet::new();
        for device_id in device_ids {
            union.extend(self.reported_mailboxes_for_device(*device_id).await?);
        }
        Ok(union)
    }

    /// The set of mailboxes `device_id` has been successfully reported to.
    pub async fn reported_mailboxes_for_device(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<BTreeSet<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT mailbox_id FROM reported_contacts WHERE device_id = ?")
                .bind(device_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{stores::create_sqlite_pool, topic::Topic};
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_initialize_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_initialize_random.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool.clone()).await.unwrap();
        let private_key = store.private_key().await.unwrap();
        let agent_id = store.agent_id().await.unwrap();
        store.ensure_initialized().await.unwrap();
        assert_eq!(
            store.private_key().await.unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().await.unwrap(), agent_id);

        drop(store);
        pool.close().await;

        let pool = create_sqlite_pool(dir.path().join("test_initialize_random.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool).await.unwrap();
        assert_eq!(
            store.private_key().await.unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().await.unwrap(), agent_id);
    }

    #[tokio::test]
    async fn test_unfetched_blob_hashes_crud() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_unfetched.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool.clone()).await.unwrap();

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
        pool.close().await;
        let pool = create_sqlite_pool(dir.path().join("test_unfetched.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool).await.unwrap();
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get(mbx_b).unwrap(), &vec![h1]);
    }

    #[tokio::test]
    async fn test_remove_unfetched_blobs_all_mailboxes() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_unfetched_all.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool.clone()).await.unwrap();

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
    async fn test_reported_contacts_crud() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_reported.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool.clone()).await.unwrap();

        let device = |n: u8| DeviceId::from(SigningKey::from_bytes(&[n; 32]).verifying_key());
        let d1 = device(1);
        let d2 = device(2);

        // d1 reported to both mailboxes; d2 only to mailbox-a.
        store
            .record_reported_devices(&[d1], &["mbx-a".into(), "mbx-b".into()])
            .await
            .unwrap();
        store
            .record_reported_devices(&[d2], &["mbx-a".into()])
            .await
            .unwrap();
        // Idempotent insert.
        store
            .record_reported_devices(&[d1], &["mbx-a".into()])
            .await
            .unwrap();

        assert_eq!(
            store.reported_mailboxes_for_device(d1).await.unwrap(),
            BTreeSet::from(["mbx-a".into(), "mbx-b".into()])
        );

        // Skip set for {d1, d2} is the intersection: only mailbox-a covers both.
        assert_eq!(
            store.mailboxes_reported_to_all(&[d1, d2]).await.unwrap(),
            BTreeSet::from(["mbx-a".into()])
        );
        // A single device's skip set is its full reported-mailbox set.
        assert_eq!(
            store.mailboxes_reported_to_all(&[d1]).await.unwrap(),
            BTreeSet::from(["mbx-a".into(), "mbx-b".into()])
        );
        // The union spans every mailbox any of the devices reached.
        assert_eq!(
            store.mailboxes_reported_to_any(&[d1, d2]).await.unwrap(),
            BTreeSet::from(["mbx-a".into(), "mbx-b".into()])
        );
        assert!(
            store
                .mailboxes_reported_to_any(&[])
                .await
                .unwrap()
                .is_empty()
        );
        // An unreported device drags the intersection to empty.
        assert!(
            store
                .mailboxes_reported_to_all(&[d1, device(3)])
                .await
                .unwrap()
                .is_empty()
        );
        // Empty input yields an empty skip set.
        assert!(
            store
                .mailboxes_reported_to_all(&[])
                .await
                .unwrap()
                .is_empty()
        );

        // Persists across reopen.
        drop(store);
        pool.close().await;
        let pool = create_sqlite_pool(dir.path().join("test_reported.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool).await.unwrap();
        assert_eq!(
            store.mailboxes_reported_to_all(&[d1, d2]).await.unwrap(),
            BTreeSet::from(["mbx-a".into()])
        );
    }

    #[tokio::test]
    async fn test_prune_expired_active_inbox_topics() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_prune_inbox_topics.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool.clone()).await.unwrap();

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

        let loaded_topics = store.get_advertised_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);

        store.prune_expired_active_inbox_topics(now).await.unwrap();
        topics.pop_first().unwrap();

        let loaded_topics = store.get_advertised_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);

        store
            .prune_expired_active_inbox_topics(more_valid)
            .await
            .unwrap();
        topics.pop_first().unwrap();

        let loaded_topics = store.get_advertised_inbox_topics().await.unwrap();
        assert_eq!(loaded_topics, topics);
    }
}
