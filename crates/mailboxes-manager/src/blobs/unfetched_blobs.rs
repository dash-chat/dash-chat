use std::{collections::BTreeMap, sync::Arc};

use mailbox_client::{
    BlobReader, MailboxId, MailboxItem, OptionalItemTraits, UnfetchedBlobTracker,
    store::MailboxStore,
};

use crate::manager::Mailboxes;

// `reconcile_unfetched_blobs` currently lives in its own file because the blob
// followup path is not yet cleanly separated from node concerns. This is a
// marker so future refactors return here.

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

    /// Refresh our dialing address with the mailbox before attempting upload.
    async fn prepare_upload_to(&self, url: &str);
}

/// One reconciliation pass: for every mailbox with unfetched blobs that is still
/// tracked, re-announce its hashes and drop the ones it reports already stored.
///
/// Drives the mailbox's `BlobDelivery` engine directly.
pub async fn reconcile_unfetched_blobs<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    source: Arc<dyn BlobSource>,
    reader: Arc<dyn BlobReader>,
    tracker: Arc<dyn UnfetchedBlobTracker>,
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
        let client = tracked.client().await;
        let Some(url) = client.url() else {
            continue; // non-HTTP mailbox (e.g. in-memory test mailbox)
        };

        let mut held = Vec::new();
        for hash in hashes {
            if mailboxes.upload_due_at(&url, hash) && reader.has_blob(hash).await {
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
        let delivery = mailbox_client::blob_delivery::BlobDelivery::new(
            client,
            Some(reader.clone()),
            tracker.clone(),
            mailboxes.upload_tracker(),
        );
        if let Err(err) = delivery.sync(held, reader.clone()).await {
            tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
        }
    }
}
