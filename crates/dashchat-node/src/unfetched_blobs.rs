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
        let reader = node.blob_reader();
        let mut held = Vec::new();
        for hash in hashes {
            if mailbox_client::toy::upload_due(&url, hash) && reader.has_blob(hash).await {
                held.push(hash);
            }
        }
        if held.is_empty() {
            continue; // nothing to upload now: in flight, backing off, or not fetched yet
        }
        // Our address changes with the network, and the mailbox fetches from us
        // with the last one we gave it.
        if let Err(err) = node.register_with_mailbox(&url).await {
            tracing::warn!(?err, mailbox = %mailbox_id, "failed to refresh our address on the mailbox");
        }
        // Upload again too: the upload that followed these blobs' message may
        // have been cut off, and the mailbox can't fetch from a phone it can't dial.
        let client =
            mailbox_client::toy::ToyMailboxClient::<crate::mailbox::MailboxOperation>::new(
                mailbox_id.clone(),
                url,
                self_endpoint,
                node.unfetched_blob_tracker(),
            )
            .with_blob_reader(reader);
        if let Err(err) = client.store_blobs(held).await {
            tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
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
