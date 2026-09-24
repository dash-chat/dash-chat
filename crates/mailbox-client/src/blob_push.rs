use std::time::Duration;

use futures::StreamExt;
use iroh::endpoint::Connection;
use iroh_blobs::protocol::{GetRequest, ObserveRequest, PushRequest};

use crate::MailboxId;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PUSH_CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

pub struct BlobPusher {
    store: iroh_blobs::api::Store,
    endpoint: iroh::Endpoint,
    alpn: Vec<u8>,
}

impl BlobPusher {
    /// `alpn` is the blobs protocol's ALPN on this device's network.
    pub fn new(store: iroh_blobs::api::Store, endpoint: iroh::Endpoint, alpn: Vec<u8>) -> Self {
        Self {
            store,
            endpoint,
            alpn,
        }
    }

    /// Push the `hashes` this device holds to `mailbox_id`, returning those the
    /// mailbox now holds whole. A blob that fails is skipped, so it does not
    /// hold back the ones after it.
    pub async fn push(
        &self,
        mailbox_id: &MailboxId,
        hashes: &[iroh_blobs::Hash],
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        let held = held_blobs(&self.store, hashes).await;
        if held.is_empty() {
            return Ok(held);
        }
        let mailbox = mailbox_server::decode_mailbox_id(mailbox_id)?;
        if mailbox == self.endpoint.id() {
            // A local hub hosted by this device serves from this device's blob
            // store, which already holds the blobs; iroh refuses to dial ourselves.
            return Ok(held);
        }
        let connection =
            tokio::time::timeout(CONNECT_TIMEOUT, self.endpoint.connect(mailbox, &self.alpn))
                .await??;
        let mut pushed = Vec::new();
        for hash in held {
            match push_blob(&self.store, &connection, hash).await {
                Ok(()) => {
                    tracing::info!(%hash, mailbox = %mailbox_id, "mailbox holds pushed blob");
                    pushed.push(hash);
                }
                Err(err) => {
                    tracing::warn!(?err, %hash, mailbox = %mailbox_id, "failed to push blob")
                }
            }
        }
        Ok(pushed)
    }
}

async fn held_blobs(
    store: &iroh_blobs::api::Store,
    hashes: &[iroh_blobs::Hash],
) -> Vec<iroh_blobs::Hash> {
    let mut held = Vec::new();
    for &hash in hashes {
        if store.has(hash).await.unwrap_or(false) {
            held.push(hash);
        }
    }
    held
}

/// A push completes once the bytes are sent, before the mailbox has stored
/// them, so the mailbox's copy is observed until it is whole. Observe streams
/// the current state and then only the ranges added since, which are folded
/// together.
async fn push_blob(
    store: &iroh_blobs::api::Store,
    connection: &Connection,
    hash: iroh_blobs::Hash,
) -> anyhow::Result<()> {
    let mut observed = store
        .remote()
        .observe(connection.clone(), ObserveRequest::new(hash));
    let Some(initial) = observed.next().await else {
        anyhow::bail!("mailbox ended the observe stream for {hash}");
    };
    let mut mailbox_copy = initial?;
    if mailbox_copy.is_complete() {
        return Ok(());
    }
    store
        .remote()
        .execute_push(
            connection.clone(),
            PushRequest::from(GetRequest::blob(hash)),
        )
        .complete()
        .await?;
    tokio::time::timeout(PUSH_CONFIRM_TIMEOUT, async {
        while let Some(update) = observed.next().await {
            mailbox_copy.update(&update?);
            if mailbox_copy.is_complete() {
                return Ok(());
            }
        }
        anyhow::bail!("mailbox ended the observe stream before holding {hash}")
    })
    .await
    .map_err(|_| anyhow::anyhow!("mailbox did not report holding pushed blob {hash}"))?
}
