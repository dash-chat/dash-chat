use std::path::PathBuf;
use std::time::Duration;

use dashchat_node::testing::TestNode;
use mailbox_local_server::LocalMailboxServer;
use mailbox_server::FetchConfig;

/// Short blob-fetch grace window for tests, so the mailbox's fetch backstop
/// fires quickly instead of waiting out the production default.
pub const TEST_UPLOAD_GRACE: Duration = Duration::from_millis(500);

/// Spawn an in-process local mailbox server that shares `relay`'s iroh endpoint
/// and blob store, wait for it to become healthy, and forward any peer address
/// registered via `/peers/register` into `relay`'s p2panda address book so the
/// shared blob fetcher can dial that peer by EndpointId. This is the in-process
/// test equivalent of `src-tauri/src/mailbox/server.rs`.
pub async fn spawn_relay_mailbox(
    relay: &TestNode,
    db_path: PathBuf,
    fetch_config: FetchConfig,
) -> LocalMailboxServer {
    let (peer_addr_tx, mut peer_addr_rx) = tokio::sync::mpsc::unbounded_channel();
    let blob_sync = relay.blob_sync_optional().expect("blob sync is enabled");
    let server = mailbox_local_server::spawn_local_mailbox_server(
        db_path,
        blob_sync.blobs.clone(),
        blob_sync.downloader(),
        relay.iroh_endpoint().await.unwrap(),
        Some(fetch_config),
        Some(TEST_UPLOAD_GRACE),
        peer_addr_tx,
    )
    .await
    .unwrap();
    mailbox_client::toy::wait_for_mailbox_health(&server.url).await;

    let relay_for_addrs = relay.clone();
    tokio::spawn(async move {
        while let Some(addr) = peer_addr_rx.recv().await {
            let _ = relay_for_addrs.insert_peer_addr(addr).await;
        }
    });

    server
}

/// A mailbox client wired the way the app wires one: the node's persisted
/// unfetched-blob tracker, streaming blob bytes read through `blob_reader`.
pub fn app_mailbox_client(
    node: &TestNode,
    mailbox_id: &mailbox_client::MailboxId,
    url: &str,
    blob_reader: std::sync::Arc<dyn mailbox_client::BlobReader>,
) -> mailbox_client::toy::ToyMailboxClient<dashchat_node::mailbox::MailboxOperation> {
    mailbox_client::toy::ToyMailboxClient::new(
        mailbox_id.clone(),
        url,
        node.endpoint_id(),
        node.unfetched_blob_tracker(),
    )
    .with_blob_reader(blob_reader)
}

/// Stands in for an app that is frozen or killed after its op reached the
/// mailbox but before the blob upload that follows it could run.
pub struct FrozenAppBlobReader;

#[async_trait::async_trait]
impl mailbox_client::BlobReader for FrozenAppBlobReader {
    async fn read_blob(&self, _hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
        std::future::pending().await
    }
}

/// Reads blob bytes through `inner`, except that the first read fails: an
/// inline upload a dropped connection cut short, which the client gives up on
/// just as it gives up on an unreadable blob.
pub struct UploadCutShortOnce {
    inner: std::sync::Arc<dyn mailbox_client::BlobReader>,
    cut: std::sync::atomic::AtomicBool,
}

impl UploadCutShortOnce {
    pub fn new(inner: std::sync::Arc<dyn mailbox_client::BlobReader>) -> Self {
        Self {
            inner,
            cut: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl mailbox_client::BlobReader for UploadCutShortOnce {
    async fn read_blob(&self, hash: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
        if !self.cut.swap(true, std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!("connection dropped mid-upload");
        }
        self.inner.read_blob(hash).await
    }
}
