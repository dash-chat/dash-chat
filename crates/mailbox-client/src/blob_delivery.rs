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

use crate::{BlobReader, BlobUploadLifecycle, HTTP_CLIENT, MailboxId, UnfetchedBlobTracker};

/// Client-side timeout for a single blob upload, larger than the default HTTP
/// timeout because a blob can be big.
const UPLOAD_BLOB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// The slowest uplink an upload is given time to finish over, so a big blob
/// from a phone on a weak connection isn't cut off by [`UPLOAD_BLOB_TIMEOUT`].
const SLOWEST_UPLINK_BYTES_PER_SEC: f64 = 32.0 * 1024.0;

/// How blob delivery to the mailbox failed. Mirrors
/// [`crate::operation_delivery::DeliveryError`]: the split is what the
/// caller's connection-health judgement needs.
#[derive(Debug, thiserror::Error)]
pub enum BlobDeliveryError {
    /// The mailbox rejected or could not complete the announce/upload
    /// exchange.
    #[error("mailbox exchange failed: {0}")]
    Mailbox(anyhow::Error),
    /// Reading the local blob store failed.
    #[error("local blob store failed: {0}")]
    Local(anyhow::Error),
}

impl BlobDeliveryError {
    /// Whether this failure says anything about the mailbox's reachability.
    pub fn is_mailbox_fault(&self) -> bool {
        !matches!(self, BlobDeliveryError::Local(_))
    }
}

/// Why a single blob upload attempt didn't succeed, so the batch can tell an
/// unreachable mailbox from a failure isolated to one blob.
#[derive(Debug)]
pub(crate) enum UploadError {
    /// The mailbox itself couldn't be reached (connection refused/reset).
    /// Every other upload in the batch will hit the same wall, so the batch
    /// stops and lets the announce/fetch backstop take over.
    #[allow(dead_code)]
    MailboxUnavailable(anyhow::Error),
    /// The mailbox is reachable but this blob didn't store (non-success
    /// status, or the request timed out mid-transfer). Isolated to this blob,
    /// so the batch skips it and keeps uploading the rest.
    Blob(anyhow::Error),
}

/// A connection-level failure means the mailbox is unreachable; anything else
/// (including a per-request timeout on one large blob) is scoped to that blob
/// so a single slow or rejected upload doesn't abort the whole batch.
fn classify_upload_error(err: reqwest::Error) -> UploadError {
    if err.is_connect() {
        UploadError::MailboxUnavailable(err.into())
    } else {
        UploadError::Blob(err.into())
    }
}

/// POST blob hashes to a mailbox's `/blobs/register-hashes`, returning the
/// subset the mailbox reports it already has stored. Set `expect_upload` when
/// the caller will stream the bytes to `/blobs/upload` right after, so the
/// mailbox defers its fetch backstop by its own fixed grace window and lets
/// that upload land first without a duplicate transfer; pass `false` to have
/// the mailbox fetch immediately (no upload is coming).
pub(crate) async fn send_register_hashes(
    base_url: &str,
    hashes: Vec<iroh_blobs::Hash>,
    sender_pubkey: iroh::EndpointId,
    expect_upload: bool,
) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let request = mailbox_server::RegisterHashesRequest {
        blob_hashes: hashes,
        sender_pubkey,
        expect_upload,
        signature: Vec::new(),
    };
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/register-hashes"))
        .json(&request)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to store blobs: {status} - {body}");
    }
    let response: mailbox_server::RegisterHashesResponse = response.json().await?;
    Ok(response.already_stored)
}

/// POST a single blob's raw bytes to a mailbox's `/blobs/upload`. The body is
/// the bytes themselves — no JSON/base64 wrapping — so the on-wire size
/// matches the blob.
pub(crate) async fn upload_blob(base_url: &str, bytes: bytes::Bytes) -> Result<(), UploadError> {
    let response = HTTP_CLIENT
        .post(format!("{base_url}/blobs/upload"))
        .timeout(UPLOAD_BLOB_TIMEOUT.max(std::time::Duration::from_secs_f64(
            bytes.len() as f64 / SLOWEST_UPLINK_BYTES_PER_SEC,
        )))
        .body(bytes)
        .send()
        .await
        .map_err(classify_upload_error)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(UploadError::Blob(anyhow::anyhow!(
            "Failed to upload blob: {status} - {body}"
        )));
    }
    Ok(())
}

/// Outbound delivery of blobs to ONE mailbox.
#[derive(Clone)]
pub struct BlobDelivery {
    mailbox_id: MailboxId,
    base_url: String,
    sender_pubkey: iroh::EndpointId,
    reader: Option<Arc<dyn BlobReader>>,
    tracker: Arc<dyn UnfetchedBlobTracker>,
    lifecycle: Arc<dyn BlobUploadLifecycle>,
}

impl BlobDelivery {
    pub fn new(
        mailbox_id: MailboxId,
        base_url: impl Into<String>,
        sender_pubkey: iroh::EndpointId,
        reader: Option<Arc<dyn BlobReader>>,
        tracker: Arc<dyn UnfetchedBlobTracker>,
        lifecycle: Arc<dyn BlobUploadLifecycle>,
    ) -> Self {
        Self {
            mailbox_id,
            base_url: base_url.into(),
            sender_pubkey,
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
        let already_stored = send_register_hashes(
            &self.base_url,
            hashes.clone(),
            self.sender_pubkey,
            expect_upload,
        )
        .await?;
        let not_stored: Vec<_> = hashes
            .into_iter()
            .filter(|h| !already_stored.contains(h))
            .collect();
        self.tracker.record(&self.mailbox_id, &not_stored).await;
        self.tracker.remove(&self.mailbox_id, &already_stored).await;
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
        // hasn't reached yet alongside it over the same uplink.
        let claimed: Vec<(iroh_blobs::Hash, u64)> = hashes
            .into_iter()
            .filter_map(|hash| Some((hash, self.lifecycle.claim_upload(&self.base_url, hash)?)))
            .collect();
        if claimed.is_empty() {
            return;
        }
        let base_url = self.base_url.clone();
        let id = self.mailbox_id.clone();
        let tracker = self.tracker.clone();
        let lifecycle = self.lifecycle.clone();
        tokio::spawn(async move {
            let mut claimed = claimed.into_iter();
            while let Some((hash, claim)) = claimed.next() {
                let bytes = match reader.read_blob(hash).await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::warn!(%hash, ?err, "failed to read blob for upload; relying on announce");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                        continue;
                    }
                };
                let size = bytes.len();
                tracing::info!(%hash, size, "uploading blob");
                match upload_blob(&base_url, bytes).await {
                    Ok(()) => {
                        tracing::info!(%hash, size, "uploaded blob");
                        tracker.remove(&id, &[hash]).await;
                        lifecycle.finish_upload(&base_url, hash, claim, true);
                    }
                    Err(UploadError::Blob(err)) => {
                        tracing::warn!(%hash, ?err, "blob upload failed; relying on announce");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                    }
                    Err(UploadError::MailboxUnavailable(err)) => {
                        tracing::warn!(%hash, ?err, "mailbox unreachable; aborting remaining uploads, relying on announce/fetch backstop");
                        lifecycle.finish_upload(&base_url, hash, claim, false);
                        claimed.by_ref().for_each(|(hash, claim)| {
                            lifecycle.finish_upload(&base_url, hash, claim, false)
                        });
                        break;
                    }
                }
            }
        });
    }
}
