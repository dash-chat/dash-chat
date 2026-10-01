use std::{collections::BTreeMap, sync::Arc};

use crate::{
    BlobReader, MailboxId, MailboxItem, OptionalItemTraits, UnfetchedBlobTracker,
    manager::Mailboxes, store::MailboxStore,
};

/// Source of blob-upload work from the node that owns the blobs.
///
/// This trait is the boundary between `mailbox-client` (which knows which
/// mailboxes are connected and how to push blobs to them) and `dashchat-node`
/// (which knows which blobs exist, where their bytes live, and how to refresh
/// the node's dialing address with a mailbox).
#[async_trait::async_trait]
pub trait BlobSource: Send + Sync + 'static {
    /// Return all blob hashes still recorded as unfetched, grouped by mailbox.
    async fn unfetched_blobs_by_mailbox(&self) -> BTreeMap<MailboxId, Vec<iroh_blobs::Hash>>;

    /// Whether the local node can read the bytes for this blob.
    async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool;

    /// Refresh our dialing address with the mailbox before attempting upload.
    async fn prepare_upload_to(&self, url: &str);
}

/// One reconciliation pass: for every mailbox with unfetched blobs that is still
/// tracked, re-announce its hashes and drop the ones it reports already stored.
///
/// This is a transitional home for the followup logic; it still constructs a
/// concrete `ToyMailboxClient` per mailbox because the generic `MailboxClient`
/// trait does not yet expose blob upload.
pub async fn reconcile_unfetched_blobs<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    source: Arc<dyn BlobSource>,
    reader: Arc<dyn BlobReader>,
    tracker: Arc<dyn UnfetchedBlobTracker>,
    endpoint_id: iroh::EndpointId,
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    let by_mailbox = source.unfetched_blobs_by_mailbox().await;
    for (mailbox_id, hashes) in by_mailbox {
        let Some(tracked) = mailboxes.tracked_mailbox(&mailbox_id).await else {
            continue; // mailbox not currently registered; retry when it returns
        };
        let Some(url) = tracked.client().await.url() else {
            continue; // non-HTTP mailbox (e.g. in-memory test mailbox)
        };

        let mut held = Vec::new();
        for hash in hashes {
            if crate::upload_scheduler::upload_due(&url, hash) && source.has_blob(hash).await {
                held.push(hash);
            }
        }
        if held.is_empty() {
            continue; // nothing to upload now: in flight, backing off, or not fetched yet
        }

        // Our address changes with the network, and the mailbox fetches from us
        // with the last one we gave it.
        source.prepare_upload_to(&url).await;

        // Upload again too: the upload that followed these blobs' message may
        // have been cut off, and the mailbox can't fetch from a phone it can't dial.
        let client = crate::backends::toy::ToyMailboxClient::<Item>::new(
            mailbox_id.clone(),
            url,
            endpoint_id,
            tracker.clone(),
        )
        .with_blob_reader(reader.clone());
        if let Err(err) = client.store_blobs(held).await {
            tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
        }
    }
}
