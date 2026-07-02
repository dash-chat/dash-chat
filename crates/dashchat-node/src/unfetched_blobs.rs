use std::sync::Arc;

use mailbox_client::{MailboxId, UnfetchedBlobTracker};

use crate::stores::LocalStore;

/// `UnfetchedBlobTracker` backed by the node's `LocalStore` table.
#[derive(Clone)]
pub struct LocalStoreBlobTracker {
    local_store: LocalStore,
}

impl LocalStoreBlobTracker {
    pub fn new(local_store: LocalStore) -> Arc<dyn UnfetchedBlobTracker> {
        Arc::new(Self { local_store })
    }
}

#[async_trait::async_trait]
impl UnfetchedBlobTracker for LocalStoreBlobTracker {
    async fn record(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]) {
        if let Err(err) = self.local_store.add_unfetched_blobs(mailbox_id, hashes).await {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to record unfetched blobs");
        }
    }
    async fn remove(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]) {
        if let Err(err) = self.local_store.remove_unfetched_blobs(mailbox_id, hashes).await {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to remove unfetched blobs");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracker_writes_through_to_local_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path().join("t.db")).await.unwrap();
        let tracker = LocalStoreBlobTracker::new(store.clone());
        let h = iroh_blobs::Hash::new([5; 32]);

        tracker.record(&"mbx".to_string(), &[h]).await;
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert_eq!(by_mailbox.get("mbx").unwrap(), &vec![h]);

        tracker.remove(&"mbx".to_string(), &[h]).await;
        let by_mailbox = store.unfetched_blobs_by_mailbox().await.unwrap();
        assert!(by_mailbox.get("mbx").is_none());
    }
}
