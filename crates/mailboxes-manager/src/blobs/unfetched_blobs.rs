use std::sync::Arc;

use mailbox_client::{
    BlobReader, MailboxItem, OptionalItemTraits, UnfetchedBlobTracker,
    blobs::blob_delivery::BlobDelivery, store::MailboxStore,
};

use crate::manager::Mailboxes;

// `reconcile_unfetched_blobs` currently lives in its own file because the blob
// followup path is not yet cleanly separated from node concerns. This is a
// marker so future refactors return here.

/// The node-side half of blob followups: refreshing our dialing address with
/// the mailbox before attempting upload. The outstanding-set read lives on
/// [`UnfetchedBlobTracker::unfetched`]; this trait is what remains of the
/// boundary once that moved.
#[async_trait::async_trait]
pub trait BlobSource: Send + Sync + 'static {
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
    for (mailbox_id, tracked) in mailboxes.registered_mailboxes().await {
        let hashes = match tracker.unfetched(&mailbox_id).await {
            Ok(hashes) => hashes,
            Err(err) => {
                tracing::error!(?err, mailbox = %mailbox_id, "failed to read unfetched blobs");
                continue;
            }
        };
        if hashes.is_empty() {
            continue;
        }
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
        let delivery = BlobDelivery::new(
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
