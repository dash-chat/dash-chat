use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use iroh_blobs::Hash as BlobHash;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use tokio::sync::{Notify, Semaphore};

use crate::MailboxId;

/// Persistent queue for ensuring blobs reach a mailbox.
pub struct BlobOutbox {
    db: Pool<Sqlite>,
    global_semaphore: Arc<Semaphore>,
    per_mailbox_limits: Mutex<HashMap<MailboxId, Arc<Semaphore>>>,
    notify: Notify,
    #[allow(dead_code)]
    config: BlobOutboxConfig,
}

#[derive(Clone, Debug)]
pub struct BlobOutboxConfig {
    pub global_concurrency: usize,
    pub per_mailbox_concurrency: usize,
    pub mailbox_unavailable_retry_interval: std::time::Duration,
    pub initial_blob_failure_backoff: std::time::Duration,
    pub max_blob_failure_backoff: std::time::Duration,
}

impl Default for BlobOutboxConfig {
    fn default() -> Self {
        Self {
            global_concurrency: 3,
            per_mailbox_concurrency: 1,
            mailbox_unavailable_retry_interval: std::time::Duration::from_secs(30),
            initial_blob_failure_backoff: std::time::Duration::from_secs(5),
            max_blob_failure_backoff: std::time::Duration::from_secs(5 * 60),
        }
    }
}

impl BlobOutbox {
    /// Open or create the outbox tables in the given SQLite database.
    pub async fn open(database_url: &str, config: BlobOutboxConfig) -> Result<Arc<Self>> {
        let db = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(database_url)
            .await?;
        Self::migrate(&db).await?;
        Ok(Arc::new(Self {
            db,
            global_semaphore: Arc::new(Semaphore::new(config.global_concurrency)),
            per_mailbox_limits: Mutex::new(HashMap::new()),
            notify: Notify::new(),
            config,
        }))
    }

    /// Ensure a blob is queued for delivery to a mailbox.
    ///
    /// If the pair is already queued or in flight, this is a no-op.
    pub async fn ensure(&self, mailbox_id: MailboxId, blob_hash: BlobHash) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO blob_outbox_jobs (mailbox_id, blob_hash, state, attempts, created_at)
            VALUES (?, ?, 'pending', 0, unixepoch('subsec'))
            ON CONFLICT(mailbox_id, blob_hash) DO UPDATE SET
                state = CASE
                    WHEN excluded.state IN ('pending', 'failed')
                    THEN excluded.state
                    ELSE blob_outbox_jobs.state
                END,
                next_attempt_at = CASE
                    WHEN excluded.state IN ('pending', 'failed')
                    THEN NULL
                    ELSE blob_outbox_jobs.next_attempt_at
                END
            "#,
        )
        .bind(mailbox_id)
        .bind(&blob_hash.as_bytes()[..])
        .execute(&self.db)
        .await?;

        self.notify.notify_one();
        Ok(())
    }

    /// Reset all pending retries immediately, for example after a network change.
    pub async fn reset_retries(&self) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE blob_outbox_jobs
            SET next_attempt_at = NULL
            WHERE state IN ('pending', 'failed')
            "#,
        )
        .execute(&self.db)
        .await?;

        self.notify.notify_one();
        Ok(())
    }

    /// Cancel all pending work for a removed mailbox.
    pub async fn cancel_mailbox(&self, mailbox_id: &MailboxId) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE blob_outbox_jobs
            SET state = 'cancelled'
            WHERE mailbox_id = ? AND state != 'done'
            "#,
        )
        .bind(mailbox_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn migrate(db: &Pool<Sqlite>) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS blob_outbox_jobs (
                id INTEGER PRIMARY KEY,
                mailbox_id TEXT NOT NULL,
                blob_hash BLOB NOT NULL,
                state TEXT NOT NULL,
                attempts INTEGER NOT NULL,
                next_attempt_at REAL,
                created_at REAL NOT NULL,
                UNIQUE(mailbox_id, blob_hash)
            );

            CREATE INDEX IF NOT EXISTS idx_blob_outbox_jobs_ready
            ON blob_outbox_jobs(state, next_attempt_at, created_at)
            WHERE state IN ('pending', 'failed');

            CREATE TABLE IF NOT EXISTS blob_outbox_job_log (
                id INTEGER PRIMARY KEY,
                job_id INTEGER NOT NULL,
                happened_at REAL NOT NULL,
                old_state TEXT,
                new_state TEXT,
                error TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_blob_outbox_job_log_job_id
            ON blob_outbox_job_log(job_id);
            "#,
        )
        .execute(db)
        .await?;

        Ok(())
    }
}
