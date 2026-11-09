//! Per-mailbox blob progress — the blob counterpart to
//! [`crate::operations::operation_sync_tracker::OperationSyncTracker`].
//!
//! Two complementary views over what each mailbox holds:
//!
//! - **Outstanding hashes** (`unfetched`, `record`, `remove`) — the
//!   unordered content-addressed set of blobs a mailbox hasn't yet
//!   confirmed. Written by the blob delivery engine as announces and
//!   uploads land; read by the reconcile pass to answer "what did we already
//!   try that hasn't landed?"
//!
//! - **Per-log heights** (`record_blob_synced`, `get_all_blob_heights_for_mailbox`)
//!   — the `topic -> author -> seq` watermark up to which a mailbox has
//!   confirmed the attached blobs. This mirrors the op tracker's
//!   watermarks, but lags them: an op is synced when the mailbox stores the
//!   body, while the blob height only advances when the mailbox confirms
//!   the referenced blob bytes.
//!
//! Both views live here because they describe the same axis (blob progress
//! per mailbox), the heights just carry more structure than the hash set.

use std::sync::Arc;

use mailbox_client::{ItemTraits, MailboxId, SeqNum, UnfetchedBlobTracker};

use crate::persistence::{BlobStore, SyncStore};

/// Persistent record of each mailbox's blob progress: outstanding hashes
/// plus per-log blob-confirmed heights.
pub struct BlobSyncTracker<T, A> {
    blob_store: Arc<dyn BlobStore>,
    sync_store: Arc<dyn SyncStore<T, A>>,
}

impl<T, A> BlobSyncTracker<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    pub(crate) fn new(
        blob_store: Arc<dyn BlobStore>,
        sync_store: Arc<dyn SyncStore<T, A>>,
    ) -> Self {
        Self {
            blob_store,
            sync_store,
        }
    }

    /// Drop every row for these hashes across all mailboxes — the node
    /// deleted the blob locally and can no longer serve it.
    pub async fn remove_all_mailboxes(&self, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        self.blob_store.remove_all_mailboxes(hashes).await
    }

    /// Every `(topic, author)`'s blob-confirmed height for one mailbox.
    /// Absent entries mean no blob has ever been confirmed for that log on
    /// this mailbox (equivalent to height 0).
    pub async fn get_all_blob_heights_for_mailbox(
        &self,
        mailbox_id: &MailboxId,
    ) -> anyhow::Result<std::collections::HashMap<T, std::collections::HashMap<A, SeqNum>>> {
        self.sync_store.get_blob_sync_state(mailbox_id).await
    }

    /// Record blob-confirmed heights for one mailbox. Heights only move
    /// forward (`max()` upsert) — the caller is the reconcile pass
    /// confirming from the mailbox's announce response which seq's blobs
    /// are now held.
    pub async fn record_blob_synced(
        &self,
        mailbox_id: &MailboxId,
        heights: &[(T, A, SeqNum)],
    ) -> anyhow::Result<()> {
        self.sync_store
            .record_blob_synced(mailbox_id, heights)
            .await
    }
}

#[async_trait::async_trait]
impl<T, A> UnfetchedBlobTracker for BlobSyncTracker<T, A>
where
    T: ItemTraits + 'static,
    A: ItemTraits + 'static,
{
    async fn record(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.blob_store.record(mailbox_id, hashes).await
    }

    async fn remove(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.blob_store.remove(mailbox_id, hashes).await
    }

    async fn unfetched(
        &self,
        mailbox_id: &MailboxId,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
        self.blob_store.unfetched(mailbox_id).await
    }
}
