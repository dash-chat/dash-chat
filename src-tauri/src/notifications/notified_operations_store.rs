use std::path::Path;
use std::time::Duration;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

const MIGRATIONS: &[&str] = &["CREATE TABLE IF NOT EXISTS notified_operations (
        hash BLOB PRIMARY KEY,
        notified_at_nanos INTEGER NOT NULL
    )"];

/// App-level persistent record of operations that have already produced a
/// user-facing notification. Both the FCM/APNs push path and the local-sync
/// path consult it so the same op never produces two banners — important on
/// Android where MessagingStyle would otherwise append the message twice into
/// the same thread.
#[derive(Clone)]
pub struct NotifiedOperationsStore {
    pool: SqlitePool,
}

impl NotifiedOperationsStore {
    pub async fn open(db_path: &Path) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(30));
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        for sql in MIGRATIONS {
            sqlx::query(sql).execute(&pool).await?;
        }
        Ok(Self { pool })
    }

    /// Records that the operation identified by `hash` has had a notification
    /// surfaced. Returns `true` iff this call inserted a new row (caller
    /// should proceed to show); `false` means another path already showed it.
    pub async fn record_notified_operation(&self, hash: p2panda_core::Hash) -> anyhow::Result<bool> {
        let now_nanos: i64 = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let result = sqlx::query("INSERT OR IGNORE INTO notified_operations (hash, notified_at_nanos) VALUES (?, ?)")
            .bind(hash.as_bytes().to_vec())
            .bind(now_nanos)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    #[allow(dead_code)]
    pub async fn has_notified_operation(&self, hash: p2panda_core::Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT notified_at_nanos FROM notified_operations WHERE hash = ?")
            .bind(hash.as_bytes().to_vec())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_from_byte(b: u8) -> p2panda_core::Hash {
        p2panda_core::Hash::from_bytes([b; 32])
    }

    async fn temp_store() -> (tempfile::TempDir, NotifiedOperationsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = NotifiedOperationsStore::open(&dir.path().join("notified_operations.db"))
            .await
            .unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn record_returns_true_then_false_for_same_op() {
        let (_dir, store) = temp_store().await;
        let hash = hash_from_byte(0xAB);
        assert!(store.record_notified_operation(hash).await.unwrap());
        assert!(!store.record_notified_operation(hash).await.unwrap());
    }

    #[tokio::test]
    async fn record_distinct_ops_independently() {
        let (_dir, store) = temp_store().await;
        let a = hash_from_byte(0x01);
        let b = hash_from_byte(0x02);
        assert!(store.record_notified_operation(a).await.unwrap());
        assert!(store.record_notified_operation(b).await.unwrap());
        assert!(!store.record_notified_operation(a).await.unwrap());
        assert!(!store.record_notified_operation(b).await.unwrap());
    }

    #[tokio::test]
    async fn has_notified_reflects_record_outcome() {
        let (_dir, store) = temp_store().await;
        let hash = hash_from_byte(0x5A);
        assert!(!store.has_notified_operation(hash).await.unwrap());
        store.record_notified_operation(hash).await.unwrap();
        assert!(store.has_notified_operation(hash).await.unwrap());
    }

    #[tokio::test]
    async fn record_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notified_operations.db");
        let hash = hash_from_byte(0x77);

        let store = NotifiedOperationsStore::open(&path).await.unwrap();
        assert!(store.record_notified_operation(hash).await.unwrap());
        drop(store);

        let store = NotifiedOperationsStore::open(&path).await.unwrap();
        assert!(!store.record_notified_operation(hash).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_records_for_same_op_produce_exactly_one_true() {
        // Models the push-vs-sync race: both paths call record_notified_operation
        // for the same op at roughly the same time; the contract is that exactly
        // one of them sees `true` (and gets to show the banner).
        //
        // Requires multi-thread flavour: the default current-thread runtime
        // interleaves spawned tasks cooperatively on one OS thread, which would
        // pass even with strictly-serial execution. We also loop so any single
        // iteration that fails to overlap doesn't mask a broken implementation.
        const RACES: usize = 50;

        let (_dir, store) = temp_store().await;

        for i in 0..RACES {
            let hash = p2panda_core::Hash::from_bytes([i as u8; 32]);
            let store_a = store.clone();
            let store_b = store.clone();
            let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
            let barrier_a = barrier.clone();
            let barrier_b = barrier.clone();
            let a = tokio::spawn(async move {
                barrier_a.wait().await;
                store_a.record_notified_operation(hash).await.unwrap()
            });
            let b = tokio::spawn(async move {
                barrier_b.wait().await;
                store_b.record_notified_operation(hash).await.unwrap()
            });
            let a = a.await.unwrap();
            let b = b.await.unwrap();
            assert!(
                a ^ b,
                "race {i}: expected exactly one of the racing callers to see true, got a={a} b={b}"
            );
        }
    }
}
