use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use iroh_blobs::Hash as BlobHash;
use sqlx::{Pool, Sqlite, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use tokio::sync::{Notify, Semaphore};

#[cfg(test)]
use std::sync::Mutex as StdMutex;

use crate::MailboxId;
use crate::blobs::outbox_store;

/// A unit of work the scheduler hands to a processor.
#[derive(Clone, Debug)]
pub struct Job {
    pub id: i64,
    pub mailbox_id: MailboxId,
    pub blob_hash: BlobHash,
}

impl From<outbox_store::JobRow> for Job {
    fn from(row: outbox_store::JobRow) -> Self {
        Self {
            id: row.id,
            mailbox_id: row.mailbox_id,
            blob_hash: row.blob_hash,
        }
    }
}

/// Persistent queue for ensuring blobs reach a mailbox.
pub struct BlobOutbox {
    db: Pool<Sqlite>,
    global_semaphore: Arc<Semaphore>,
    per_mailbox_limits: Mutex<HashMap<MailboxId, Arc<Semaphore>>>,
    notify: Arc<Notify>,
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
        let opts = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
        let db = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;
        outbox_store::migrate(&db).await?;
        Ok(Arc::new(Self {
            db,
            global_semaphore: Arc::new(Semaphore::new(config.global_concurrency)),
            per_mailbox_limits: Mutex::new(HashMap::new()),
            notify: Arc::new(Notify::new()),
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
        outbox_store::cancel_mailbox_jobs(&self.db, mailbox_id).await
    }

    /// Spawn the scheduler loop that drives the outbox.
    ///
    /// `process` is called for every ready job. On success the job is marked
    /// done; on failure it is marked failed and retried later.
    pub fn spawn_scheduler<F, Fut>(&self, process: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(Job) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send,
    {
        let db = self.db.clone();
        let notify = self.notify.clone();
        let initial_backoff = self.config.initial_blob_failure_backoff;

        tokio::spawn(async move {
            loop {
                // Drain any work that is already due, then wait for new notifications.
                while let Some(row) = outbox_store::select_next_ready(&db).await.unwrap_or(None) {
                    let job_id = row.id;
                    if !outbox_store::try_claim_job(&db, job_id)
                        .await
                        .unwrap_or(false)
                    {
                        continue;
                    }

                    let job = Job::from(row);

                    if (process)(job).await.is_ok() {
                        let _ = outbox_store::mark_done(&db, job_id).await;
                    } else {
                        let next_attempt_at = (std::time::SystemTime::now() + initial_backoff)
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64();
                        let _ = outbox_store::mark_failed(&db, job_id, next_attempt_at).await;

                        let notify = notify.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(initial_backoff).await;
                            notify.notify_one();
                        });
                    }
                }

                notify.notified().await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn example_job() -> (MailboxId, BlobHash) {
        ("test-mbx".to_string(), BlobHash::new(b"test blob"))
    }

    fn recording_processor(
        processed: Arc<StdMutex<Vec<Job>>>,
    ) -> impl Fn(Job) -> std::future::Ready<Result<()>> {
        move |job| {
            processed.lock().unwrap().push(job);
            std::future::ready(Ok(()))
        }
    }

    fn flaky_processor(
        attempts: Arc<StdMutex<usize>>,
        failures_before_success: usize,
        recorded: Arc<StdMutex<Vec<Job>>>,
    ) -> impl Fn(Job) -> std::future::Ready<Result<()>> {
        move |job| {
            let mut attempts = attempts.lock().unwrap();
            *attempts += 1;
            let attempt = *attempts;
            recorded.lock().unwrap().push(job);
            if attempt > failures_before_success {
                std::future::ready(Ok(()))
            } else {
                std::future::ready(Err(anyhow::anyhow!("boom")))
            }
        }
    }

    async fn open_in_memory() -> Arc<BlobOutbox> {
        open_in_memory_with_config(BlobOutboxConfig::default()).await
    }

    async fn open_in_memory_with_config(config: BlobOutboxConfig) -> Arc<BlobOutbox> {
        BlobOutbox::open("sqlite::memory:", config).await.unwrap()
    }

    fn fast_backoff_config() -> BlobOutboxConfig {
        let mut config = BlobOutboxConfig::default();
        config.initial_blob_failure_backoff = Duration::from_millis(100);
        config
    }

    async fn open_temp_file() -> (tempfile::TempDir, String) {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("outbox.db");
        let database_url = format!("sqlite://{}", db_path.to_string_lossy());
        (tmp, database_url)
    }

    fn spawn_recording_scheduler(outbox: &Arc<BlobOutbox>) -> (tokio::task::JoinHandle<()>, Arc<StdMutex<Vec<Job>>>) {
        let processed: Arc<StdMutex<Vec<Job>>> = Arc::new(StdMutex::new(Vec::new()));
        let handle = outbox.spawn_scheduler(recording_processor(processed.clone()));
        (handle, processed)
    }

    fn spawn_flaky_scheduler(
        outbox: &Arc<BlobOutbox>,
        failures_before_success: usize,
    ) -> (
        tokio::task::JoinHandle<()>,
        Arc<StdMutex<Vec<Job>>>,
        Arc<StdMutex<usize>>,
    ) {
        let attempts = Arc::new(StdMutex::new(0usize));
        let recorded = Arc::new(StdMutex::new(Vec::new()));
        let handle = outbox.spawn_scheduler(flaky_processor(
            attempts.clone(),
            failures_before_success,
            recorded.clone(),
        ));
        (handle, recorded, attempts)
    }

    async fn wait_for_jobs(
        processed: &Arc<StdMutex<Vec<Job>>>,
        timeout: Duration,
        message: &str,
    ) {
        let delivered = tokio::time::timeout(timeout, async {
            loop {
                if !processed.lock().unwrap().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;

        assert!(delivered.is_ok(), "{message}");
    }

    async fn wait_for_attempts(
        attempts: &Arc<StdMutex<usize>>,
        target: usize,
        timeout: Duration,
        message: &str,
    ) {
        let delivered = tokio::time::timeout(timeout, async {
            loop {
                if *attempts.lock().unwrap() >= target {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;

        assert!(delivered.is_ok(), "{message}");
    }

    async fn expect_one_job(
        processed: &Arc<StdMutex<Vec<Job>>>,
        mailbox_id: &MailboxId,
        hash: BlobHash,
        message: &str,
    ) {
        wait_for_jobs(processed, Duration::from_secs(2), message).await;
        let jobs = processed.lock().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].mailbox_id, *mailbox_id);
        assert_eq!(jobs[0].blob_hash, hash);
    }

    fn assert_jobs_match(jobs: &Vec<Job>, mailbox_id: &MailboxId, hash: BlobHash) {
        for job in jobs {
            assert_eq!(job.mailbox_id, *mailbox_id);
            assert_eq!(job.blob_hash, hash);
        }
    }

    #[tokio::test]
    async fn scheduler_delivers_pending_job_to_processor() {
        let outbox = open_in_memory().await;
        let (scheduler, processed) = spawn_recording_scheduler(&outbox);

        let (mailbox_id, hash) = example_job();
        outbox.ensure(mailbox_id.clone(), hash).await.unwrap();

        expect_one_job(
            &processed,
            &mailbox_id,
            hash,
            "scheduler did not deliver the job in time",
        )
        .await;
        scheduler.abort();
    }

    #[tokio::test]
    async fn scheduler_processes_existing_jobs_on_startup() {
        let (_tmp, database_url) = open_temp_file().await;

        let producer = BlobOutbox::open(&database_url, BlobOutboxConfig::default())
            .await
            .unwrap();

        let (mailbox_id, hash) = example_job();
        producer.ensure(mailbox_id.clone(), hash).await.unwrap();
        drop(producer);

        let consumer = BlobOutbox::open(&database_url, BlobOutboxConfig::default())
            .await
            .unwrap();
        let (scheduler, processed) = spawn_recording_scheduler(&consumer);

        expect_one_job(
            &processed,
            &mailbox_id,
            hash,
            "scheduler did not process the pre-existing job in time",
        )
        .await;
        scheduler.abort();
    }

    #[tokio::test]
    async fn scheduler_retries_failed_jobs_after_backoff() {
        let outbox = open_in_memory_with_config(fast_backoff_config()).await;
        let (scheduler, recorded, attempts) = spawn_flaky_scheduler(&outbox, 1);

        let (mailbox_id, hash) = example_job();
        outbox.ensure(mailbox_id.clone(), hash).await.unwrap();

        // First attempt fails quickly.
        wait_for_jobs(
            &recorded,
            Duration::from_secs(2),
            "first attempt did not run",
        )
        .await;

        // After backoff the scheduler retries and the second attempt succeeds.
        wait_for_attempts(
            &attempts,
            2,
            Duration::from_secs(10),
            "scheduler did not retry the failed job in time",
        )
        .await;

        scheduler.abort();

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_jobs_match(&recorded, &mailbox_id, hash);
    }
}
