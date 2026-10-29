//! Per-mailbox record of which blob hashes the mailbox still lacks — the
//! blob counterpart to [`crate::operations::operation_sync_tracker::OperationSyncTracker`].
//!
//! The outstanding set is unordered and content-addressed, so where the op
//! tracker stores one watermark per (topic, author), this stores a row per
//! (mailbox, hash). Writes come from the blob delivery engine; reads drive
//! the reconcile pass.
//!
//! Unlike the op tracker there is no watch surface yet: nothing subscribes,
//! so the tracker is a thin `Result`-forwarding wrapper over the store. Add
//! watch channels when a consumer exists, updating them from the write batch
//! rather than re-reading.

use std::sync::Arc;

use mailbox_client::{MailboxId, UnfetchedBlobTracker};

use crate::persistence::BlobStore;

/// Persistent record of which blobs each mailbox has not yet confirmed.
pub struct BlobSyncTracker {
    store: Arc<dyn BlobStore>,
}

impl BlobSyncTracker {
    pub(crate) fn new(store: Arc<dyn BlobStore>) -> Self {
        Self { store }
    }

    /// Drop every row for these hashes across all mailboxes — the node
    /// deleted the blob locally and can no longer serve it.
    pub async fn remove_all_mailboxes(&self, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        self.store.remove_all_mailboxes(hashes).await
    }
}

#[async_trait::async_trait]
impl UnfetchedBlobTracker for BlobSyncTracker {
    async fn record(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.store.record(mailbox_id, hashes).await
    }

    async fn remove(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.store.remove(mailbox_id, hashes).await
    }

    async fn unfetched(
        &self,
        mailbox_id: &MailboxId,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
        self.store.unfetched(mailbox_id).await
    }
}
