use p2panda::Hash;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{BTreeSet, HashMap};

use crate::TopicId;

const MIGRATIONS: &[&str] = &["CREATE TABLE IF NOT EXISTS tombstones (
        topic_id BLOB NOT NULL,
        op_hash BLOB NOT NULL,
        reason TEXT NOT NULL,
        PRIMARY KEY (topic_id, op_hash)
    )"];

/// Why an operation was tombstoned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TombstoneReason {
    DeletedForEveryone,
    DeletedForMe,
}

impl TombstoneReason {
    fn to_db(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .expect("TombstoneReason serializes to a string")
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_value(serde_json::Value::String(
            value.to_owned(),
        ))?)
    }
}

/// Which operations were tombstoned, and why.
///
/// Unlike the [`crate::stores::OpProjection`] this cannot be rebuilt from the
/// [`crate::stores::OpStore`]: tombstoning an operation drops its body, and
/// with it the edit pointers and the validity of the delete that tell which
/// operations a delete covers. It must outlive a purge of the projection.
#[derive(Clone)]
pub struct TombstoneStore {
    pool: SqlitePool,
}

impl TombstoneStore {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        for sql in MIGRATIONS {
            sqlx::query(*sql).execute(&pool).await?;
        }
        Ok(Self { pool })
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
            .map(|(bytes,)| hash_from_db(bytes))
            .collect()
    }

    /// The reason `op_hash` was tombstoned in `topic`, or `None` if it isn't
    /// tombstoned.
    pub async fn tombstone_reason(
        &self,
        topic: TopicId,
        op_hash: Hash,
    ) -> anyhow::Result<Option<TombstoneReason>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT reason FROM tombstones WHERE topic_id = ? AND op_hash = ?")
                .bind(topic.as_bytes().to_vec())
                .bind(op_hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        row.map(|(reason,)| TombstoneReason::from_db(&reason))
            .transpose()
    }

    /// Every tombstone in `topic`, paired with its reason. The frontend uses
    /// this to drop delete-for-me messages (and their edits) from view while
    /// keeping the delete-for-everyone placeholders.
    pub async fn tombstones(
        &self,
        topic: TopicId,
    ) -> anyhow::Result<HashMap<Hash, TombstoneReason>> {
        let rows: Vec<(Vec<u8>, String)> =
            sqlx::query_as("SELECT op_hash, reason FROM tombstones WHERE topic_id = ?")
                .bind(topic.as_bytes().to_vec())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(bytes, reason)| Ok((hash_from_db(bytes)?, TombstoneReason::from_db(&reason)?)))
            .collect()
    }

    /// Record an operation hash in the per-topic tombstone set with the reason
    /// it was tombstoned.
    ///
    /// Reasons are first-write-wins, with one deliberate
    /// exception: a `DeletedForMe` upgrades an existing `DeletedForEveryone` (so
    /// a message I deleted for myself vanishes even when it's also deleted for
    /// everyone). Every other combination keeps the existing reason.
    pub(crate) async fn add_tombstone(
        &self,
        topic: TopicId,
        op_hash: Hash,
        reason: TombstoneReason,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO tombstones (topic_id, op_hash, reason) VALUES (?, ?, ?)
             ON CONFLICT(topic_id, op_hash) DO UPDATE SET reason = excluded.reason
             WHERE excluded.reason = ? AND tombstones.reason = ?",
        )
        .bind(topic.as_bytes().to_vec())
        .bind(op_hash.as_bytes().to_vec())
        .bind(reason.to_db())
        // Only the specific DeletedForEveryone -> DeletedForMe upgrade replaces
        // an existing reason.
        .bind(TombstoneReason::DeletedForMe.to_db())
        .bind(TombstoneReason::DeletedForEveryone.to_db())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn hash_from_db(bytes: Vec<u8>) -> anyhow::Result<Hash> {
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("tombstone op_hash is not 32 bytes"))?;
    Ok(Hash::from_bytes(arr))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{Topic, stores::create_sqlite_pool, topic::kind};

    #[tokio::test]
    async fn test_tombstones_per_topic() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(
            dir.path().join("test_tombstones.db"),
            &sqlite_gate::SqliteGate::default(),
        )
        .await
        .unwrap()
        .pool()
        .clone();
        let db = TombstoneStore::new(pool.clone()).await.unwrap();

        let topic_a = *Topic::<kind::Untyped>::new([1; 32]);
        let topic_b = *Topic::<kind::Untyped>::new([2; 32]);
        let hash1 = Hash::digest(b"op1");
        let hash2 = Hash::digest(b"op2");

        assert!(!db.is_tombstoned(topic_a, hash1).await.unwrap());

        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        db.add_tombstone(topic_a, hash2, TombstoneReason::DeletedForMe)
            .await
            .unwrap();
        db.add_tombstone(topic_b, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        // Adding the same hash again is idempotent.
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();

        assert!(db.is_tombstoned(topic_a, hash1).await.unwrap());
        assert!(db.is_tombstoned(topic_a, hash2).await.unwrap());
        // The reason is recorded per tombstone.
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForEveryone)
        );
        assert_eq!(
            db.tombstone_reason(topic_a, hash2).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );
        // Tombstones are scoped per-topic: hash2 in topic_a does not leak into topic_b.
        assert!(db.is_tombstoned(topic_b, hash1).await.unwrap());
        assert!(!db.is_tombstoned(topic_b, hash2).await.unwrap());
        assert_eq!(db.tombstone_reason(topic_b, hash2).await.unwrap(), None);

        // Delete-for-me always wins: it upgrades an existing delete-for-everyone,
        // and a later delete-for-everyone never downgrades it back.
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForMe)
            .await
            .unwrap();
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );
        db.add_tombstone(topic_a, hash1, TombstoneReason::DeletedForEveryone)
            .await
            .unwrap();
        assert_eq!(
            db.tombstone_reason(topic_a, hash1).await.unwrap(),
            Some(TombstoneReason::DeletedForMe)
        );

        assert_eq!(
            db.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
        assert_eq!(
            db.tombstoned_hashes(topic_b).await.unwrap(),
            maplit::btreeset![hash1]
        );

        // Tombstones persist across reopening the database.
        drop(db);
        pool.close().await;

        let pool = create_sqlite_pool(
            dir.path().join("test_tombstones.db"),
            &sqlite_gate::SqliteGate::default(),
        )
        .await
        .unwrap()
        .pool()
        .clone();
        let db = TombstoneStore::new(pool).await.unwrap();
        assert!(db.is_tombstoned(topic_a, hash1).await.unwrap());
        assert_eq!(
            db.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
    }
}
