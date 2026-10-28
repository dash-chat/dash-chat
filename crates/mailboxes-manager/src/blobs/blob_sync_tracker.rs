//! Per-mailbox record of which blob hashes the mailbox still lacks — the
//! blob counterpart to [`crate::operations::operation_sync_tracker::OperationSyncTracker`].
//!
//! The outstanding set is unordered and content-addressed, so where the op
//! tracker stores one watermark per (topic, author), this stores a row per
//! (mailbox, hash). Writes come from the blob delivery engine; reads drive
//! the reconcile pass and (via the watch) UI observability.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mailbox_client::{MailboxId, UnfetchedBlobTracker};
use tokio::sync::watch;

use crate::persistence::BlobStore;

/// Watch-based tracker for which blobs each mailbox has not yet confirmed,
/// backed by a persistence store (SQLite in production, in-memory for tests).
pub struct BlobSyncTracker {
    store: Arc<dyn BlobStore>,
    all_ids_tx: watch::Sender<BTreeSet<MailboxId>>,
    per_mailbox: tokio::sync::Mutex<BTreeMap<MailboxId, watch::Sender<BTreeSet<iroh_blobs::Hash>>>>,
}

impl BlobSyncTracker {
    pub(crate) fn new(store: Arc<dyn BlobStore>, initial_ids: BTreeSet<MailboxId>) -> Self {
        let (all_ids_tx, _) = watch::channel(initial_ids);
        Self {
            store,
            all_ids_tx,
            per_mailbox: tokio::sync::Mutex::new(BTreeMap::new()),
        }
    }

    /// Subscribe to the set of mailbox ids that have ever recorded blob state.
    pub fn all_mailbox_ids(&self) -> watch::Receiver<BTreeSet<MailboxId>> {
        self.all_ids_tx.subscribe()
    }

    /// Subscribe to a mailbox's outstanding blob set. Lazily creates the
    /// watch on first call, seeded from persisted state.
    pub async fn unfetched_state(
        &self,
        mailbox: &MailboxId,
    ) -> anyhow::Result<watch::Receiver<BTreeSet<iroh_blobs::Hash>>> {
        let mut per_mailbox = self.per_mailbox.lock().await;
        if let Some(tx) = per_mailbox.get(mailbox) {
            return Ok(tx.subscribe());
        }
        let initial = self.store.unfetched(mailbox).await?.into_iter().collect();
        let (tx, rx) = watch::channel(initial);
        per_mailbox.insert(mailbox.clone(), tx);
        Ok(rx)
    }

    /// Drop every row for these hashes across all mailboxes — the node
    /// deleted the blob locally and can no longer serve it.
    pub async fn remove_all_mailboxes(&self, hashes: &[iroh_blobs::Hash]) -> anyhow::Result<()> {
        self.store.remove_all_mailboxes(hashes).await?;
        // Snapshot the senders first: no lock held across the store reads.
        let senders: Vec<(MailboxId, watch::Sender<BTreeSet<iroh_blobs::Hash>>)> = {
            let per_mailbox = self.per_mailbox.lock().await;
            per_mailbox
                .iter()
                .map(|(id, tx)| (id.clone(), tx.clone()))
                .collect()
        };
        for (id, tx) in senders {
            let current: BTreeSet<_> = self.store.unfetched(&id).await?.into_iter().collect();
            tx.send_if_modified(|set| {
                if *set != current {
                    *set = current.clone();
                    true
                } else {
                    false
                }
            });
        }
        Ok(())
    }

    async fn refresh_watch(&self, mailbox: &MailboxId, current: BTreeSet<iroh_blobs::Hash>) {
        self.all_ids_tx
            .send_if_modified(|ids| ids.insert(mailbox.clone()));
        let per_mailbox = self.per_mailbox.lock().await;
        if let Some(tx) = per_mailbox.get(mailbox) {
            tx.send_if_modified(|set| {
                if *set != current {
                    *set = current;
                    true
                } else {
                    false
                }
            });
        }
    }
}

#[async_trait::async_trait]
impl UnfetchedBlobTracker for BlobSyncTracker {
    async fn record(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.store.record(mailbox_id, hashes).await?;
        let current = self
            .store
            .unfetched(mailbox_id)
            .await?
            .into_iter()
            .collect();
        self.refresh_watch(mailbox_id, current).await;
        Ok(())
    }

    async fn remove(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> Result<(), anyhow::Error> {
        self.store.remove(mailbox_id, hashes).await?;
        let current = self
            .store
            .unfetched(mailbox_id)
            .await?
            .into_iter()
            .collect();
        self.refresh_watch(mailbox_id, current).await;
        Ok(())
    }

    async fn unfetched(
        &self,
        mailbox_id: &MailboxId,
    ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
        self.store.unfetched(mailbox_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::mem::MemBlobStore;

    fn tracker() -> BlobSyncTracker {
        BlobSyncTracker::new(Arc::new(MemBlobStore::default()), Default::default())
    }

    #[tokio::test]
    async fn record_and_remove_round_trip() {
        let tracker = tracker();
        let mbx = "mbx".to_string();
        let h1 = iroh_blobs::Hash::new([1; 32]);
        let h2 = iroh_blobs::Hash::new([2; 32]);

        tracker.record(&mbx, &[h1, h2]).await.unwrap();
        assert_eq!(tracker.unfetched(&mbx).await.unwrap().len(), 2);

        tracker.remove(&mbx, &[h1]).await.unwrap();
        assert_eq!(tracker.unfetched(&mbx).await.unwrap(), vec![h2]);

        tracker.remove(&mbx, &[h2]).await.unwrap();
        assert!(tracker.unfetched(&mbx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn unfetched_state_watch_updates_on_writes() {
        let tracker = tracker();
        let mbx = "mbx".to_string();
        let h = iroh_blobs::Hash::new([7; 32]);

        let mut rx = tracker.unfetched_state(&mbx).await.unwrap();
        assert!(rx.borrow().is_empty());

        tracker.record(&mbx, &[h]).await.unwrap();
        assert!(rx.has_changed().unwrap());
        assert!(rx.borrow_and_update().contains(&h));

        tracker.remove(&mbx, &[h]).await.unwrap();
        assert!(rx.has_changed().unwrap());
        assert!(rx.borrow_and_update().is_empty());
    }

    #[tokio::test]
    async fn remove_all_mailboxes_clears_every_mailbox() {
        let tracker = tracker();
        let h = iroh_blobs::Hash::new([9; 32]);
        tracker.record(&"a".to_string(), &[h]).await.unwrap();
        tracker.record(&"b".to_string(), &[h]).await.unwrap();

        tracker.remove_all_mailboxes(&[h]).await.unwrap();

        assert!(
            tracker
                .unfetched(&"a".to_string())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            tracker
                .unfetched(&"b".to_string())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn all_mailbox_ids_observes_new_mailboxes() {
        let tracker = tracker();
        assert!(tracker.all_mailbox_ids().borrow().is_empty());

        tracker
            .record(&"mbx".to_string(), &[iroh_blobs::Hash::new([1; 32])])
            .await
            .unwrap();
        assert!(tracker.all_mailbox_ids().borrow().contains("mbx"));
    }
}
