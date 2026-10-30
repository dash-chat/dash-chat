//! Per-mailbox delivery of blobs.
//!
//! This module provides `BlobDelivery`: the blob counterpart to
//! [`crate::operations::operation_delivery::OperationDelivery`]. One instance is the
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
    pub async fn publish(
        &self,
        hashes: Vec<iroh_blobs::Hash>,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        self.announce_and_upload(hashes, self.reader.clone()).await
    }

    /// Reconcile a known-outstanding set with the mailbox: re-announce the
    /// hashes and push any bytes the mailbox still lacks. Uses the engine's
    /// reader: a sync that can't serve bytes would announce hashes the
    /// mailbox then fetches from a node that doesn't have them.
    ///
    /// Returns the hashes the mailbox confirmed it already holds at announce
    /// time. The detached upload may land more after this returns — the
    /// caller (typically a reconcile pass that maps blobs back to op seq
    /// numbers) uses the returned set to advance its per-mailbox
    /// blob-confirmed heights, and picks up upload-confirmed blobs on the
    /// next pass.
    pub async fn sync(
        &self,
        hashes: Vec<iroh_blobs::Hash>,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        self.announce_and_upload(hashes, self.reader.clone()).await
    }

    async fn announce_and_upload(
        &self,
        mut hashes: Vec<iroh_blobs::Hash>,
        reader: Option<Arc<dyn BlobReader>>,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
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
        if let Some(reader) = reader {
            self.spawn_upload(reader, not_stored);
        }
        Ok(already_stored)
    }

    /// Spawn a detached best-effort task that streams each blob's bytes to
    /// the mailbox, one at a time (so at most one blob is held in memory).
    /// Every blob that uploads successfully is removed from the unfetched
    /// tracker (the mailbox now holds it). We keep going through the batch as
    /// long as uploads are feasible: a blob we can't read or that the mailbox
    /// rejects is left in the tracker and skipped, but the moment the mailbox
    /// itself is unreachable we stop — the remaining blobs stay queued for
    /// the mailbox's fetch backstop rather than burning through the batch
    /// against a dead endpoint.
    fn spawn_upload(&self, reader: Arc<dyn BlobReader>, hashes: Vec<iroh_blobs::Hash>) {
        // Claimed up front, so a later sync doesn't start the ones this task
        // hasn't reached yet alongside it over the same uplink. The lifecycle
        // keys claims on the mailbox's URL — the shared upload tracker's
        // granularity — falling back to its id for mailboxes without one
        // (in-memory tests). The unfetched-blob tracker keys on the id:
        // that's the registry key the reconcile pass reads by, and the two
        // must never be conflated.
        let claim_key = self.client.url().unwrap_or_else(|| self.client.id());
        let id = self.client.id();
        let claimed: Vec<(iroh_blobs::Hash, u64)> = hashes
            .into_iter()
            .filter_map(|hash| Some((hash, self.lifecycle.claim_upload(&claim_key, hash)?)))
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
                        lifecycle.finish_upload(&claim_key, hash, claim, false);
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
                        lifecycle.finish_upload(&claim_key, hash, claim, true);
                    }
                    Err(crate::BlobUploadError::MailboxUnavailable(err)) => {
                        tracing::warn!(%hash, ?err, "mailbox unreachable; aborting remaining uploads, relying on announce/fetch backstop");
                        lifecycle.finish_upload(&claim_key, hash, claim, false);
                        claimed.by_ref().for_each(|(hash, claim)| {
                            lifecycle.finish_upload(&claim_key, hash, claim, false)
                        });
                        break;
                    }
                    Err(crate::BlobUploadError::Blob(err)) => {
                        tracing::warn!(%hash, ?err, "blob upload failed; relying on announce");
                        lifecycle.finish_upload(&claim_key, hash, claim, false);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Msg;
    use crate::{FetchRequest, FetchResponse, MailboxClient, MailboxId, PublishResponse};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;

    /// A tracker that actually records, keyed by mailbox id — the registry
    /// key the reconcile pass reads by.
    #[derive(Clone, Default)]
    struct RecordingTracker {
        sets: Arc<Mutex<BTreeMap<MailboxId, BTreeSet<iroh_blobs::Hash>>>>,
        emptied: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    }

    #[async_trait::async_trait]
    impl UnfetchedBlobTracker for RecordingTracker {
        async fn record(
            &self,
            mailbox_id: &MailboxId,
            hashes: &[iroh_blobs::Hash],
        ) -> Result<(), anyhow::Error> {
            self.sets
                .lock()
                .unwrap()
                .entry(mailbox_id.clone())
                .or_default()
                .extend(hashes.iter().copied());
            Ok(())
        }

        async fn remove(
            &self,
            mailbox_id: &MailboxId,
            hashes: &[iroh_blobs::Hash],
        ) -> Result<(), anyhow::Error> {
            if let Some(set) = self.sets.lock().unwrap().get_mut(mailbox_id) {
                for hash in hashes {
                    set.remove(hash);
                }
            }
            let empty = self
                .sets
                .lock()
                .unwrap()
                .get(mailbox_id)
                .is_none_or(|set| set.is_empty());
            if empty && let Some(tx) = self.emptied.lock().unwrap().take() {
                let _ = tx.send(());
            }
            Ok(())
        }

        async fn unfetched(
            &self,
            mailbox_id: &MailboxId,
        ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
            Ok(self
                .sets
                .lock()
                .unwrap()
                .get(mailbox_id)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default())
        }
    }

    /// A client with a URL distinct from its id — the shape every real
    /// mailbox has, and the shape under which the keys were conflated.
    struct UrlClient {
        id: MailboxId,
        url: String,
    }

    #[async_trait::async_trait]
    impl MailboxClient<Msg> for UrlClient {
        fn id(&self) -> MailboxId {
            self.id.clone()
        }

        fn url(&self) -> Option<String> {
            Some(self.url.clone())
        }

        async fn publish(&self, _ops: Vec<Msg>) -> Result<PublishResponse<Msg>, anyhow::Error> {
            Ok(PublishResponse::default())
        }

        async fn fetch(
            &self,
            _request: FetchRequest<Msg>,
        ) -> Result<FetchResponse<Msg>, anyhow::Error> {
            Ok(FetchResponse(Default::default()))
        }

        async fn announce_blob_hashes(
            &self,
            _hashes: Vec<iroh_blobs::Hash>,
            _expect_upload: bool,
        ) -> Result<Vec<iroh_blobs::Hash>, anyhow::Error> {
            Ok(Vec::new()) // nothing already stored
        }

        async fn upload_blob(&self, _bytes: bytes::Bytes) -> Result<(), crate::BlobUploadError> {
            Ok(())
        }
    }

    struct StaticReader {
        hash: iroh_blobs::Hash,
    }

    #[async_trait::async_trait]
    impl BlobReader for StaticReader {
        async fn read_blob(&self, _hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
            Ok(bytes::Bytes::from_static(b"blob bytes"))
        }

        async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool {
            hash == self.hash
        }
    }

    /// Regression: the tracker's key is the mailbox id (what reconcile reads
    /// by), while the lifecycle claims by URL — collapsing them meant a
    /// successful upload never cleared the recorded row.
    #[tokio::test]
    async fn successful_upload_clears_the_row_recorded_under_the_mailbox_id() {
        let hash = iroh_blobs::Hash::new(b"keyed blob");
        let (emptied_tx, emptied_rx) = tokio::sync::oneshot::channel();
        let tracker = RecordingTracker::default();
        *tracker.emptied.lock().unwrap() = Some(emptied_tx);
        let delivery = BlobDelivery::<Msg>::new(
            Arc::new(UrlClient {
                id: "mailbox-id".to_string(),
                url: "http://mailbox-url".to_string(),
            }),
            Some(Arc::new(StaticReader { hash })),
            Arc::new(tracker.clone()),
            crate::testing::noop_upload_lifecycle(),
        );

        delivery.publish(vec![hash]).await.unwrap();
        // The spawned upload runs detached; the tracker signals when the row
        // is cleared (upload returns, then remove runs).
        emptied_rx.await.unwrap();

        assert!(
            tracker
                .unfetched(&"mailbox-id".to_string())
                .await
                .unwrap()
                .is_empty(),
            "a successful upload must clear the row recorded under the mailbox id"
        );
    }
}
