use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{MailboxId, OpaqHash};

/// An entry in the blob publish queue: a blob that needs to be uploaded to a specific mailbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobPublishEntry {
    /// Unique ID for this queue entry, used for acknowledgement.
    pub id: u64,
    /// Hash of the blob to publish.
    pub blob_hash: OpaqHash,
    /// The mailbox to publish to.
    pub mailbox_id: MailboxId,
}

/// A persistent queue of blobs that need to be published to remote mailboxes.
///
/// The blob data itself lives in [`MailboxStore`] — this queue only tracks
/// *which* blobs need to be sent *where*. Entries survive process restarts.
///
/// Implementors should ensure:
/// - `enqueue` is idempotent for the same `(blob_hash, mailbox_id)` pair
/// - `dequeue_batch` returns the oldest entries first (FIFO)
/// - `ack` removes a completed entry; `nack` returns it to the queue for retry
#[async_trait::async_trait]
pub trait BlobPublishQueue: Clone + Send + Sync + 'static {
    /// Add a blob to the publish queue for a specific mailbox.
    /// Should be idempotent: re-enqueueing the same (hash, mailbox) pair is a no-op.
    async fn enqueue(&self, blob_hash: OpaqHash, mailbox_id: MailboxId) -> anyhow::Result<()>;

    /// Dequeue up to `limit` entries for processing.
    /// Returned entries should not be returned again by subsequent calls
    /// until `nack` is called (i.e. they are "in-flight").
    async fn dequeue_batch(&self, limit: usize) -> anyhow::Result<Vec<BlobPublishEntry>>;

    /// Acknowledge successful publication — removes the entry permanently.
    async fn ack(&self, id: u64) -> anyhow::Result<()>;

    /// Negative-acknowledge — return the entry to the queue for retry.
    async fn nack(&self, id: u64) -> anyhow::Result<()>;

    /// Number of entries waiting (not in-flight).
    async fn pending_count(&self) -> anyhow::Result<usize>;
}

/// In-memory (non-persistent) blob publish queue.
/// Suitable for testing or as a temporary stand-in until a persistent
/// implementation (e.g. backed by redb) is provided.
#[derive(Clone)]
pub struct MemBlobPublishQueue {
    next_id: Arc<AtomicU64>,
    entries: Arc<Mutex<Vec<(BlobPublishEntry, bool)>>>,
}

impl MemBlobPublishQueue {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            entries: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl BlobPublishQueue for MemBlobPublishQueue {
    async fn enqueue(&self, blob_hash: OpaqHash, mailbox_id: MailboxId) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().await;
        // Idempotent: skip if already queued for this (hash, mailbox)
        if entries
            .iter()
            .any(|(e, _)| e.blob_hash == blob_hash && e.mailbox_id == mailbox_id)
        {
            return Ok(());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        entries.push((
            BlobPublishEntry {
                id,
                blob_hash,
                mailbox_id,
            },
            false,
        ));
        Ok(())
    }

    async fn dequeue_batch(&self, limit: usize) -> anyhow::Result<Vec<BlobPublishEntry>> {
        let mut entries = self.entries.lock().await;
        let mut batch = vec![];
        for (entry, in_flight) in entries.iter_mut() {
            if !*in_flight {
                *in_flight = true;
                batch.push(entry.clone());
                if batch.len() >= limit {
                    break;
                }
            }
        }
        Ok(batch)
    }

    async fn ack(&self, id: u64) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().await;
        entries.retain(|(e, _)| e.id != id);
        Ok(())
    }

    async fn nack(&self, id: u64) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().await;
        if let Some((_, in_flight)) = entries.iter_mut().find(|(e, _)| e.id == id) {
            *in_flight = false;
        }
        Ok(())
    }

    async fn pending_count(&self) -> anyhow::Result<usize> {
        let entries = self.entries.lock().await;
        Ok(entries.iter().filter(|(_, in_flight)| !in_flight).count())
    }
}
