use anyhow::Result;
use iroh_blobs::Hash as BlobHash;
use sqlx::{Pool, Sqlite};

use crate::MailboxId;

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

pub async fn cancel_jobs_for_mailbox(db: &Pool<Sqlite>, mailbox_id: &MailboxId) -> Result<()> {
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
