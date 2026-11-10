//! Pushing blobs to a mailbox over iroh instead of HTTP.

use iroh_blobs::protocol::{ChunkRanges, ChunkRangesSeq, PushRequest};

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
    /// id. The mailbox's dialing address must already be discoverable by the
    /// endpoint (the node learns it from the mailbox's `/health` response).
    pub async fn push_blob(
        &self,
        mailbox_id: &MailboxId,
        hash: iroh_blobs::Hash,
    ) -> Result<(), BlobUploadError> {
        let endpoint_id =
            mailbox_server::decode_mailbox_id(mailbox_id).map_err(BlobUploadError::Blob)?;
        let size = match self.store.status(hash).await {
            Ok(iroh_blobs::api::proto::BlobStatus::Complete { size }) => size,
            _ => 0,
        };
        let conn = self
            .endpoint
            .connect(endpoint_id, &self.alpn)
            .await
            .map_err(|err| BlobUploadError::MailboxUnavailable(anyhow::anyhow!(err)))?;
        let request = PushRequest::new(hash, ChunkRangesSeq::from_ranges([ChunkRanges::all()]));
        tokio::time::timeout(
            blob_transfer_timeout(size),
            self.store.remote().execute_push(conn, request).complete(),
        )
        .await
        .map_err(|_| BlobUploadError::Blob(anyhow::anyhow!("blob push timed out")))?
        .map_err(|err| BlobUploadError::Blob(anyhow::anyhow!("blob push failed: {err:?}")))?;
        Ok(())
    }
}
