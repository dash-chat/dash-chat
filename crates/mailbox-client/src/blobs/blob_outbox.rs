use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use iroh_blobs::Hash as BlobHash;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use tokio::sync::{Notify, Semaphore};

use crate::MailboxId;
use crate::blobs::outbox_store;

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
        outbox_store::migrate(&db).await?;
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
        outbox_store::ensure_job(&self.db, &mailbox_id, blob_hash).await?;
        self.notify.notify_one();
        Ok(())
    }

    /// Reset all pending retries immediately, for example after a network change.
    pub async fn reset_retries(&self) -> Result<()> {
        outbox_store::reset_retries(&self.db).await?;
        self.notify.notify_one();
        Ok(())
    }

    /// Cancel all pending work for a removed mailbox.
    pub async fn cancel_mailbox(&self, mailbox_id: &MailboxId) -> Result<()> {
        outbox_store::cancel_jobs_for_mailbox(&self.db, mailbox_id).await
    }
}
