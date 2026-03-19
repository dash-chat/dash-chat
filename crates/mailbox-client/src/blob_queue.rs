use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{MailboxId, OpaqHash};

mod mem_impl;
pub use mem_impl::MemBlobPublishQueue;

#[cfg(feature = "blob-publish-queue-redb")]
mod redb_impl;

#[cfg(feature = "blob-publish-queue-redb")]
pub use redb_impl::RedbBlobPublishQueue;

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
