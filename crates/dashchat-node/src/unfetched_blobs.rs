use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use mailbox_client::UnfetchedBlobTracker;
use mailboxes_manager::BlobSource;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::node::Node;

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
