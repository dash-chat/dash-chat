//! Pushing blobs to a mailbox over iroh instead of HTTP.

use futures::StreamExt;
use iroh_blobs::protocol::{ChunkRanges, ChunkRangesSeq, ObserveRequest, PushRequest};

use crate::{BlobUploadError, MailboxId, blob_transfer_timeout};

/// Pushes blobs from a local iroh-blobs store to a mailbox's iroh endpoint.
///
/// The mailbox imports the chunk-validated bao stream as it arrives, so an
/// interrupted push leaves validated chunks in its store — unlike the HTTP
/// upload, which discards a partial body. Pushes currently send the whole
/// blob; announcing only the ranges the mailbox still lacks (so retries
/// resume) is a later step.
#[derive(Clone)]
pub struct IrohPusher {
    endpoint: iroh::Endpoint,
    store: iroh_blobs::api::Store,
    alpn: Vec<u8>,
}

impl IrohPusher {
    pub fn new(endpoint: iroh::Endpoint, store: iroh_blobs::api::Store, alpn: Vec<u8>) -> Self {
        Self {
            endpoint,
            store,
            alpn,
        }
    }

    /// Push the whole blob to the mailbox whose `MailboxId` is its endpoint
    /// id, returning once the mailbox reports it holds the complete blob.
    /// The mailbox's dialing address must already be discoverable by the
    /// endpoint (the node learns it from the mailbox's `/health` response).
    pub async fn push_blob(
        &self,
        mailbox_id: &MailboxId,
        hash: iroh_blobs::Hash,
    ) -> Result<(), BlobUploadError> {
        let endpoint_id =
            mailbox_server::decode_mailbox_id(mailbox_id).map_err(BlobUploadError::Blob)?;
        // A blob we don't fully hold can't be pushed: fail fast
        // rather than paying a dial plus a full transfer timeout.
        let size = match self.store.status(hash).await {
            Ok(iroh_blobs::api::proto::BlobStatus::Complete { size }) => size,
            _ => {
                return Err(BlobUploadError::Blob(anyhow::anyhow!(
                    "blob is not fully present locally"
                )));
            }
        };
        // The dial shares the transfer budget: an unbounded connect would
        // stall every later blob in a serial delivery batch.
        tokio::time::timeout(blob_transfer_timeout(size), async {
            let conn = self
                .endpoint
                .connect(endpoint_id, &self.alpn)
                .await
                .map_err(|err| BlobUploadError::MailboxUnavailable(anyhow::anyhow!(err)))?;
            self.push_and_confirm(conn, hash).await
        })
        .await
        .map_err(|_| BlobUploadError::Blob(anyhow::anyhow!("blob push timed out")))?
    }

    async fn push_and_confirm(
        &self,
        conn: iroh::endpoint::Connection,
        hash: iroh_blobs::Hash,
    ) -> Result<(), BlobUploadError> {
        let remote = self.store.remote();
        // A push writes nothing back on success, so confirmation comes from
        // watching the mailbox's bitfield over a second stream: it sends the
        // current state at once and then live updates, and complete means the
        // import finished. Started before the push so no update is missed.
        let mut bitfields = remote.observe(conn.clone(), ObserveRequest::new(hash));
        let request = PushRequest::new(hash, ChunkRangesSeq::from_ranges([ChunkRanges::all()]));
        remote
            .execute_push(conn, request)
            .complete()
            .await
            .map_err(|err| BlobUploadError::Blob(anyhow::anyhow!("blob push failed: {err:?}")))?;
        while let Some(bitfield) = bitfields.next().await {
            let bitfield = bitfield.map_err(|err| {
                BlobUploadError::Blob(anyhow::anyhow!("blob observe failed: {err}"))
            })?;
            if bitfield.is_complete() {
                return Ok(());
            }
        }
        Err(BlobUploadError::Blob(anyhow::anyhow!(
            "mailbox closed the observe stream before the blob completed"
        )))
    }
}
