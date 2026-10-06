use anyhow::Result;
use iroh_blobs::Hash as BlobHash;
use sqlx::{Pool, Sqlite};

use crate::MailboxId;

/// A row from `blob_outbox_jobs` that the scheduler loop can work on.
pub struct JobRow {
    pub id: i64,
    pub mailbox_id: MailboxId,
    pub blob_hash: BlobHash,
}

#[derive(sqlx::FromRow)]
struct RawJobRow {
    id: i64,
    mailbox_id: String,
    blob_hash: Vec<u8>,
}

/// Run the self-contained `CREATE TABLE IF NOT EXISTS` migrations for the blob
/// outbox tables. Safe to call even when other modules already own tables in
/// the same database.
pub async fn migrate(db: &Pool<Sqlite>) -> Result<()> {
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
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Ensure a blob is queued for delivery to a mailbox.
///
/// If the pair is already queued or backing off, this resets it to pending
/// so it is picked up again. If it is already done or currently in flight,
/// the state is left unchanged.
pub async fn ensure_job(
    db: &Pool<Sqlite>,
    mailbox_id: &MailboxId,
    blob_hash: BlobHash,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO blob_outbox_jobs (mailbox_id, blob_hash, state, attempts, created_at)
        VALUES (?, ?, 'pending', 0, unixepoch('subsec'))
        ON CONFLICT(mailbox_id, blob_hash) DO UPDATE SET
            state = CASE
                WHEN blob_outbox_jobs.state IN ('pending', 'failed')
                THEN 'pending'
                ELSE blob_outbox_jobs.state
            END,
            next_attempt_at = CASE
                WHEN blob_outbox_jobs.state IN ('pending', 'failed')
                THEN NULL
                ELSE blob_outbox_jobs.next_attempt_at
            END
        "#,
    )
    .bind(mailbox_id)
    .bind(&blob_hash.as_bytes()[..])
    .execute(db)
    .await?;

    Ok(())
}

pub async fn reset_retries(db: &Pool<Sqlite>) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE blob_outbox_jobs
        SET next_attempt_at = NULL
        WHERE state IN ('pending', 'failed')
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn cancel_mailbox_jobs(db: &Pool<Sqlite>, mailbox_id: &MailboxId) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE blob_outbox_jobs
        SET state = 'cancelled'
        WHERE mailbox_id = ? AND state != 'done'
        "#,
    )
    .bind(mailbox_id)
    .execute(db)
    .await?;

    Ok(())
}

/// Pick the oldest ready job. Returns `None` when no work is due.
pub async fn select_next_ready(db: &Pool<Sqlite>) -> Result<Option<JobRow>> {
    let row = sqlx::query_as::<_, RawJobRow>(
        r#"
        SELECT id, mailbox_id, blob_hash
        FROM blob_outbox_jobs
        WHERE state IN ('pending', 'failed')
          AND (next_attempt_at IS NULL OR next_attempt_at <= unixepoch('subsec'))
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(
        |RawJobRow {
             id,
             mailbox_id,
             blob_hash,
         }| {
            let bytes: [u8; 32] = blob_hash.try_into().unwrap_or_default();
            JobRow {
                id,
                mailbox_id,
                blob_hash: BlobHash::from_bytes(bytes),
            }
        },
    ))
}

/// Claim a ready job by moving it to the `uploading` state.
///
/// Returns `true` if the row was owned by this call. A concurrent worker may
/// have already claimed it, in which case this returns `false`.
pub async fn try_claim_job(db: &Pool<Sqlite>, id: i64) -> Result<bool> {
    let rows = sqlx::query(
        r#"
        UPDATE blob_outbox_jobs
        SET state = 'uploading'
        WHERE id = ? AND state IN ('pending', 'failed')
        "#,
    )
    .bind(id)
    .execute(db)
    .await?
    .rows_affected();

    Ok(rows > 0)
}

/// Mark a job as successfully completed.
pub async fn mark_done(db: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE blob_outbox_jobs
        SET state = 'done'
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(db)
    .await?;

    Ok(())
}

/// Mark a job as failed and schedule its next attempt.
pub async fn mark_failed(db: &Pool<Sqlite>, id: i64, next_attempt_at: f64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE blob_outbox_jobs
        SET state = 'failed', attempts = attempts + 1, next_attempt_at = ?
        WHERE id = ?
        "#,
    )
    .bind(next_attempt_at)
    .bind(id)
    .execute(db)
    .await?;

    Ok(())
}
