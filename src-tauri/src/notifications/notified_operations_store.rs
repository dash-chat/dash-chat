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
    pub async fn record_notified_operation(
        &self,
        hash: p2panda_core::Hash,
    ) -> anyhow::Result<bool> {
        let now_nanos: i64 = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO notified_operations (hash, notified_at_nanos) VALUES (?, ?)",
        )
        .bind(hash.as_bytes().to_vec())
        .bind(now_nanos)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    #[allow(dead_code)]
    pub async fn has_notified_operation(&self, hash: p2panda_core::Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT notified_at_nanos FROM notified_operations WHERE hash = ?")
                .bind(hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }
}
