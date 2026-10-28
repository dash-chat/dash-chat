//! Per-mailbox delivery of blobs.
//!
//! This module provides `BlobDelivery`: the blob counterpart to
//! [`crate::operation_delivery::OperationDelivery`]. One instance is the
//! delivery relationship with one mailbox: it announces blob hashes (so the
//! mailbox knows to fetch them as a backstop) and streams the bytes the
//! mailbox reports lacking, with claims and retry backoffs handled through
//! the injected [`BlobUploadLifecycle`].
//!
//! Unlike operations, blobs are unordered and content-addressed: the
//! outstanding set is a list of hashes, not a watermark, and delivery has a
//! third party — the mailbox can fetch the bytes from us over iroh when a
//! push never lands. The announce (`publish`) therefore has two phases, both
//! visible: tell the mailbox what exists, then upload what it lacks.

use std::sync::Arc;

use crate::{BlobReader, BlobUploadLifecycle, UnfetchedBlobTracker};

/// Outbound delivery of blobs to ONE mailbox, driving the client's blob
/// primitives (`announce_blob_hashes` / `upload_blob`).
#[derive(Clone)]
pub struct BlobDelivery<Item: crate::MailboxItem> {
    client: Arc<dyn crate::MailboxClient<Item>>,
    reader: Option<Arc<dyn BlobReader>>,
    tracker: Arc<dyn UnfetchedBlobTracker>,
    lifecycle: Arc<dyn BlobUploadLifecycle>,
}

impl<Item: crate::MailboxItem> BlobDelivery<Item> {
    pub fn new(
        client: Arc<dyn crate::MailboxClient<Item>>,
        reader: Option<Arc<dyn BlobReader>>,
        tracker: Arc<dyn UnfetchedBlobTracker>,
        lifecycle: Arc<dyn BlobUploadLifecycle>,
    ) -> Self {
        Self {
            client,
            reader,
            tracker,
            lifecycle,
        }
    }

    /// Attach a blob-bytes source so `publish` streams blob bytes to the
    /// mailbox after announcing their hashes. Without a reader the client
    /// only announces hashes and the mailbox fetches the bytes from us.
    pub fn with_blob_reader(mut self, reader: Arc<dyn BlobReader>) -> Self {
        self.reader = Some(reader);
        self
    }

    /// Announce blob hashes to the mailbox and reconcile the unfetched
    /// tracker, then push the bytes the mailbox still needs in a detached
    /// best-effort task.
    ///
    /// The announce goes first and is awaited: it is what atomically locks in
    /// the delivery (the mailbox now knows to fetch these hashes as a
    /// backstop). Only then do we stream the bytes the mailbox reported it
    /// lacks — spawned, so a batch of large blobs never stalls the caller,
    /// and scoped to `not_stored`, so we never re-upload blobs the mailbox
    /// already holds.
    pub async fn publish(&self, hashes: Vec<iroh_blobs::Hash>) -> anyhow::Result<()> {
        self.announce_and_upload(hashes, self.reader.clone()).await
    }

    /// Reconcile a known-outstanding set with the mailbox: re-announce the
    /// hashes and push any bytes the mailbox still lacks. `reader` is
    /// required — a sync that can't serve bytes would announce hashes the
    /// mailbox then fetches from a node that doesn't have them.
    pub async fn sync(
        &self,
        hashes: Vec<iroh_blobs::Hash>,
        reader: Arc<dyn BlobReader>,
    ) -> anyhow::Result<()> {
        self.announce_and_upload(hashes, Some(reader)).await
    }

    async fn announce_and_upload(
        &self,
        mut hashes: Vec<iroh_blobs::Hash>,
        reader: Option<Arc<dyn BlobReader>>,
    ) -> anyhow::Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }
        // A forwarded op's blob this device hasn't fetched is left out: the
        // mailbox would dial us for bytes we don't have.
        if let Some(reader) = &reader {
            let mut held = Vec::new();
            for hash in hashes {
                if reader.has_blob(hash).await {
                    held.push(hash);
                }
            }
            hashes = held;
        }
        // Tell the mailbox to defer its fetch backstop only when we can
        // actually stream the bytes; a reader-less publish never uploads, so
        // the mailbox should fetch from us right away.
        let expect_upload = reader.is_some();
        let already_stored = self
            .client
            .announce_blob_hashes(hashes.clone(), expect_upload)
            .await?;
        let not_stored: Vec<_> = hashes
            .into_iter()
            .filter(|h| !already_stored.contains(h))
            .collect();
        let id = self.client.id();
        // Bookkeeping must not fail delivery: the announce already happened.
        if let Err(err) = self.tracker.record(&id, &not_stored).await {
            tracing::error!(?err, mailbox = %id, "failed to record unfetched blobs");
        }
        if let Err(err) = self.tracker.remove(&id, &already_stored).await {
            tracing::error!(?err, mailbox = %id, "failed to clear stored blobs");
        }
        self.spawn_upload(reader, not_stored);
        Ok(())
    }

    /// Spawn a detached best-effort task that streams each blob's bytes to
    /// the mailbox, one at a time (so at most one blob is held in memory).
    /// Every blob that uploads successfully is removed from the unfetched
    /// tracker (the mailbox now holds it). We keep going through the batch as
    /// long as uploads are feasible: a blob we can't read or that the mailbox
    /// rejects is left in the tracker and skipped, but the moment the mailbox
    /// itself is unreachable we stop — the remaining blobs stay queued for
    /// the mailbox's fetch backstop rather than burning through the batch
    /// against a dead endpoint. No-op when no blob reader is configured.
    fn spawn_upload(&self, reader: Option<Arc<dyn BlobReader>>, hashes: Vec<iroh_blobs::Hash>) {
        let Some(reader) = reader else {
            return;
        };
        // Claimed up front, so a later sync doesn't start the ones this task
        // hasn't reached yet alongside it over the same uplink. Claims key on
        // the mailbox's URL — the shared tracker's granularity — falling back
        // to its id for mailboxes without one (in-memory tests).
        let id = self.client.url().unwrap_or_else(|| self.client.id());
        let claimed: Vec<(iroh_blobs::Hash, u64)> = hashes
            .into_iter()
            .filter_map(|hash| Some((hash, self.lifecycle.claim_upload(&id, hash)?)))
            .collect();
        if claimed.is_empty() {
            return;
        }
        let client = self.client.clone();
        let tracker = self.tracker.clone();
        let lifecycle = self.lifecycle.clone();
        tokio::spawn(async move {
            let mut claimed = claimed.into_iter();
            while let Some((hash, claim)) = claimed.next() {
                let bytes = match reader.read_blob(hash).await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::warn!(%hash, ?err, "failed to read blob for upload; relying on announce");
                        lifecycle.finish_upload(&id, hash, claim, false);
                        continue;
                    }
                };
                let size = bytes.len();
                tracing::info!(%hash, size, "uploading blob");
                match client.upload_blob(bytes).await {
                    Ok(()) => {
                        tracing::info!(%hash, size, "uploaded blob");
                        if let Err(err) = tracker.remove(&id, &[hash]).await {
                            tracing::error!(?err, mailbox = %id, "failed to clear uploaded blob");
                        }
                        lifecycle.finish_upload(&id, hash, claim, true);
                    }
                    Err(crate::BlobUploadError::MailboxUnavailable(err)) => {
                        tracing::warn!(%hash, ?err, "mailbox unreachable; aborting remaining uploads, relying on announce/fetch backstop");
                        lifecycle.finish_upload(&id, hash, claim, false);
                        claimed.by_ref().for_each(|(hash, claim)| {
                            lifecycle.finish_upload(&id, hash, claim, false)
                        });
                        break;
                    }
                    Err(crate::BlobUploadError::Blob(err)) => {
                        tracing::warn!(%hash, ?err, "blob upload failed; relying on announce");
                        lifecycle.finish_upload(&id, hash, claim, false);
                    }
                }
            }
        });
    }
}
