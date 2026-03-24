use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::Mutex;

use crate::blob_queue::BlobPublishEntry;
use crate::blob_queue::BlobPublishQueue;
use crate::{MailboxId, OpaqHash};

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
