use std::sync::Arc;
use std::time::Duration;

use mailbox_client::{MailboxId, UnfetchedBlobTracker};
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

/// One reconciliation pass: for every mailbox with unfetched blobs that is still
/// tracked, re-announce its hashes and drop the ones it reports already stored.
pub async fn followup_unfetched_blobs_once(node: &Node) {
    let by_mailbox = match node.local_store.unfetched_blobs_by_mailbox().await {
        Ok(m) => m,
        Err(err) => {
            tracing::error!(?err, "failed to read unfetched blobs");
            return;
        }
    };
    let self_endpoint = node.endpoint_id();
    for (mailbox_id, hashes) in by_mailbox {
        let Some(tracked) = node.mailboxes.tracked_mailbox(&mailbox_id).await else {
            continue; // mailbox not currently registered; retry when it returns
        };
        let Some(url) = tracked.client().await.url() else {
            continue; // non-HTTP mailbox (e.g. in-memory test mailbox)
        };
        if hashes.is_empty() {
            continue; // no unfetched blobs to re-announce
        }
        // Re-announce so the mailbox re-registers these hashes for fetching. No
        // upload follows here, so ask it to fetch immediately rather than deferring
        // by its grace window. The empty op_ref marks this as a re-announce: we
        // track unfetched blobs per mailbox, not per operation, so we have no
        // reference to offer and must not mint one.
        match mailbox_client::toy::send_register_hashes(
            &url,
            hashes,
            String::new(),
            self_endpoint,
            false,
        )
        .await
        {
            Ok(already_stored) => {
                if let Err(err) = node
                    .local_store
                    .remove_unfetched_blobs(&mailbox_id, &already_stored)
                    .await
                {
                    tracing::error!(?err, mailbox = %mailbox_id, "failed to reconcile unfetched blobs");
                }
                let already_stored_count = already_stored.len();
                tracing::info!(mailbox = %mailbox_id, %already_stored_count, "re-sent unfetched blobs");
            }
            Err(err) => {
                tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
            }
        }
    }
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
