use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use iroh::endpoint::Connection;
use iroh_blobs::api::proto::Bitfield;
use iroh_blobs::protocol::{GetRequest, ObserveRequest, PushRequest};
use mailbox_client::{BlobPushQueue, MailboxId};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::node::Node;
use crate::stores::LocalStore;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PUSH_CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

pub struct LocalStoreBlobPushQueue {
    local_store: LocalStore,
    blobs: Option<iroh_blobs::BlobsProtocol>,
    trigger: Arc<Notify>,
}

impl LocalStoreBlobPushQueue {
    pub fn new(
        local_store: LocalStore,
        blobs: Option<iroh_blobs::BlobsProtocol>,
        trigger: Arc<Notify>,
    ) -> Self {
        Self {
            local_store,
            blobs,
            trigger,
        }
    }
}

#[async_trait::async_trait]
impl BlobPushQueue for LocalStoreBlobPushQueue {
    /// Queue only the blobs this node holds: a node forwarding someone else's
    /// operation before fetching its blobs has nothing to push.
    async fn enqueue(&self, mailbox_id: &MailboxId, hashes: &[iroh_blobs::Hash]) {
        let Some(blobs) = &self.blobs else {
            return;
        };
        let held = held_blobs(blobs, hashes).await;
        if held.is_empty() {
            return;
        }
        if let Err(err) = self
            .local_store
            .add_pending_blob_pushes(mailbox_id, &held)
            .await
        {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to queue blob pushes");
            return;
        }
        self.trigger.notify_one();
    }
}

async fn held_blobs(
    blobs: &iroh_blobs::BlobsProtocol,
    hashes: &[iroh_blobs::Hash],
) -> Vec<iroh_blobs::Hash> {
    let mut held = Vec::new();
    for &hash in hashes {
        if blobs.has(hash).await.unwrap_or(false) {
            held.push(hash);
        }
    }
    held
}

pub async fn push_pending_blobs_once(node: &Node) {
    let Some(blob_sync) = node.blob_sync_optional() else {
        return;
    };
    let Ok(endpoint) = node.iroh_endpoint().await else {
        return;
    };
    let by_mailbox = match node.local_store.pending_blob_pushes_by_mailbox().await {
        Ok(by_mailbox) => by_mailbox,
        Err(err) => {
            tracing::error!(?err, "failed to read pending blob pushes");
            return;
        }
    };
    for (mailbox_id, hashes) in by_mailbox {
        if !node.mailboxes.is_tracked(&mailbox_id).await {
            continue;
        }
        if let Err(err) =
            push_to_mailbox(node, &blob_sync.blobs, &endpoint, &mailbox_id, hashes).await
        {
            tracing::warn!(?err, mailbox = %mailbox_id, "failed to push blobs to mailbox; retrying later");
        }
    }
}

async fn push_to_mailbox(
    node: &Node,
    blobs: &iroh_blobs::BlobsProtocol,
    endpoint: &iroh::Endpoint,
    mailbox_id: &MailboxId,
    hashes: Vec<iroh_blobs::Hash>,
) -> anyhow::Result<()> {
    let alpn =
        p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, node.config.network_id);
    let mailbox = mailbox_server::decode_mailbox_id(mailbox_id)?;
    if mailbox == node.endpoint_id() {
        // The node's own local hub serves from the node's blob store, which
        // already holds the blobs; iroh refuses to dial ourselves.
        for hash in hashes {
            node.local_store
                .remove_pending_blob_push(mailbox_id, hash)
                .await?;
        }
        return Ok(());
    }
    let connection =
        tokio::time::timeout(CONNECT_TIMEOUT, endpoint.connect(mailbox, &alpn)).await??;
    for hash in hashes {
        if blobs.has(hash).await? {
            push_blob(blobs.store(), &connection, hash).await?;
            tracing::info!(%hash, mailbox = %mailbox_id, "mailbox holds pushed blob");
        }
        node.local_store
            .remove_pending_blob_push(mailbox_id, hash)
            .await?;
    }
    Ok(())
}

/// A push completes once the bytes are sent, before the mailbox has stored
/// them, so the mailbox's copy is observed until it is whole.
async fn push_blob(
    store: &iroh_blobs::api::Store,
    connection: &Connection,
    hash: iroh_blobs::Hash,
) -> anyhow::Result<()> {
    if mailbox_holds(store, connection, hash).await? {
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
    tokio::time::timeout(
        PUSH_CONFIRM_TIMEOUT,
        until_mailbox_holds(store, connection, hash),
    )
    .await
    .map_err(|_| anyhow::anyhow!("mailbox did not report holding pushed blob {hash}"))?
}

async fn mailbox_holds(
    store: &iroh_blobs::api::Store,
    connection: &Connection,
    hash: iroh_blobs::Hash,
) -> anyhow::Result<bool> {
    let mut observed = store
        .remote()
        .observe(connection.clone(), ObserveRequest::new(hash));
    match observed.next().await {
        Some(state) => Ok(state?.is_complete()),
        None => anyhow::bail!("mailbox ended the observe stream for {hash}"),
    }
}

/// Observe streams the current state and then only the ranges added since,
/// which are folded together.
async fn until_mailbox_holds(
    store: &iroh_blobs::api::Store,
    connection: &Connection,
    hash: iroh_blobs::Hash,
) -> anyhow::Result<()> {
    let mut observed = store
        .remote()
        .observe(connection.clone(), ObserveRequest::new(hash));
    let mut mailbox_copy = Bitfield::empty();
    while let Some(update) = observed.next().await {
        mailbox_copy.update(&update?);
        if mailbox_copy.is_complete() {
            return Ok(());
        }
    }
    anyhow::bail!("mailbox ended the observe stream before holding {hash}")
}

pub fn spawn_blob_push_task(
    node: Node,
    interval: Duration,
    trigger: Arc<Notify>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut network = network_watch::network_change();
        let mut registered_mailboxes = node.mailboxes.active_mailbox_ids();
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = trigger.notified() => {}
                _ = network.recv() => {}
                _ = registered_mailboxes.changed() => {}
            }
            push_pending_blobs_once(&node).await;
        }
    })
}
