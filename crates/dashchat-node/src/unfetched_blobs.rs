use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use mailbox_client::{BlobSource, MailboxId, UnfetchedBlobTracker};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::node::Node;
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
        if let Err(err) = self
            .local_store
            .add_unfetched_blobs(mailbox_id, hashes)
            .await
        {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to record unfetched blobs");
        }
    }
    async fn remove(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]) {
        if let Err(err) = self
            .local_store
            .remove_unfetched_blobs(mailbox_id, hashes)
            .await
        {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to remove unfetched blobs");
        }
    }
}

struct NodeBlobSource {
    node: Node,
}

impl NodeBlobSource {
    fn new(node: Node) -> Arc<dyn BlobSource> {
        Arc::new(Self { node })
    }
}

#[async_trait::async_trait]
impl BlobSource for NodeBlobSource {
    async fn unfetched_blobs_by_mailbox(&self) -> BTreeMap<MailboxId, Vec<iroh_blobs::Hash>> {
        match self.node.local_store.unfetched_blobs_by_mailbox().await {
            Ok(m) => m,
            Err(err) => {
                tracing::error!(?err, "failed to read unfetched blobs");
                BTreeMap::new()
            }
        }
    }

    async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool {
        self.node.blob_reader().has_blob(hash).await
    }

    async fn prepare_upload_to(&self, url: &str) {
        if let Err(err) = self.node.register_with_mailbox(url).await {
            tracing::warn!(?err, "failed to refresh our address on the mailbox");
        }
    }
}

/// One reconciliation pass: for every mailbox with unfetched blobs that is still
/// tracked, re-announce its hashes and drop the ones it reports already stored.
pub async fn followup_unfetched_blobs_once(node: &Node) {
    node.mailboxes
        .reconcile_unfetched_blobs(
            NodeBlobSource::new(node.clone()),
            node.blob_reader(),
            node.unfetched_blob_tracker(),
            node.endpoint_id(),
        )
        .await;
}

/// Spawn the loop that runs `followup_unfetched_blobs_once` on `interval` and
/// immediately whenever `trigger` is notified (startup, unpause, network change).
pub fn spawn_unfetched_blob_followup_task(
    node: Node,
    interval: Duration,
    trigger: Arc<Notify>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Fire once immediately on startup.
        followup_unfetched_blobs_once(&node).await;
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = trigger.notified() => {}
            }
            followup_unfetched_blobs_once(&node).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracker_writes_through_to_local_store() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::stores::create_sqlite_pool(dir.path().join("t.db"))
            .await
            .unwrap();
        let store = LocalStore::new(pool).await.unwrap();
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
