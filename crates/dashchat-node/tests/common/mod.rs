use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dashchat_node::mailbox::MailboxOperation;
use dashchat_node::testing::TestNode;
use mailbox_client::toy::ToyMailboxClient;
use mailbox_local_server::LocalMailboxServer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

/// Spawn an in-process local mailbox server that shares `relay`'s iroh endpoint
/// and blob store and wait for it to become healthy. This is the in-process
/// test equivalent of `src-tauri/src/mailbox/server.rs`.
pub async fn spawn_relay_mailbox(relay: &TestNode, db_path: PathBuf) -> LocalMailboxServer {
    let blob_sync = relay.blob_sync_optional().expect("blob sync is enabled");
    let server = mailbox_local_server::spawn_local_mailbox_server(
        db_path,
        blob_sync.blobs.clone(),
        relay.iroh_endpoint().await.unwrap(),
    )
    .await
    .unwrap();
    mailbox_client::toy::wait_for_mailbox_health(&server.url).await;
    server
}

/// A standalone mailbox server with its own iroh endpoint and blob store, as
/// the cloud mailbox runs. Stops when dropped.
pub struct StandaloneMailbox {
    pub url: String,
    pub id: mailbox_client::MailboxId,
    pub endpoint_addr: iroh::EndpointAddr,
    _dir: tempfile::TempDir,
    _stop: tokio::sync::oneshot::Sender<()>,
}

pub async fn spawn_standalone_mailbox() -> StandaloneMailbox {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("mailbox.redb");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let signal = async move {
            let _ = stopped.await;
        };
        if let Err(err) = mailbox_server::spawn_server(
            db_path,
            listener,
            None,
            None,
            None,
            *dashchat_utils::NETWORK_ID,
            signal,
        )
        .await
        {
            tracing::error!("standalone test mailbox failed: {err:?}");
        }
    });
    mailbox_client::toy::wait_for_mailbox_health(&url).await;
    let health = dashchat_node::mailbox::fetch_mailbox_health(&url)
        .await
        .unwrap();
    StandaloneMailbox {
        url,
        id: health.mailbox_id,
        endpoint_addr: health.endpoint_addr,
        _dir: dir,
        _stop: stop,
    }
}

/// A TCP proxy in front of a mailbox's HTTP port, standing in for a sender's
/// uplink to it: what the sender sends through it can be slowed down, and the
/// connections open through it dropped.
pub struct MailboxLink {
    pub url: String,
    upload_bytes_per_sec: Arc<AtomicU64>,
    dropped: tokio::sync::watch::Sender<u64>,
}

impl MailboxLink {
    pub async fn spawn(mailbox_url: &str) -> Self {
        let target = mailbox_url
            .strip_prefix("http://")
            .expect("mailbox url is plain http")
            .to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let upload_bytes_per_sec = Arc::new(AtomicU64::new(0));
        let (dropped, _) = tokio::sync::watch::channel(0u64);
        let link = Self {
            url,
            upload_bytes_per_sec: upload_bytes_per_sec.clone(),
            dropped: dropped.clone(),
        };
        tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                let Ok(upstream) = tokio::net::TcpStream::connect(&target).await else {
                    continue;
                };
                tokio::spawn(forward(
                    client,
                    upstream,
                    upload_bytes_per_sec.clone(),
                    dropped.subscribe(),
                ));
            }
        });
        link
    }

    pub fn throttle_uploads(&self, bytes_per_sec: u64) {
        self.upload_bytes_per_sec
            .store(bytes_per_sec, Ordering::Relaxed);
    }

    pub fn unthrottle(&self) {
        self.upload_bytes_per_sec.store(0, Ordering::Relaxed);
    }

    pub fn drop_connections(&self) {
        self.dropped.send_modify(|drops| *drops += 1);
    }
}

async fn forward(
    client: tokio::net::TcpStream,
    upstream: tokio::net::TcpStream,
    upload_bytes_per_sec: Arc<AtomicU64>,
    mut dropped: tokio::sync::watch::Receiver<u64>,
) {
    let (client_read, mut client_write) = client.into_split();
    let (mut upstream_read, upstream_write) = upstream.into_split();
    tokio::select! {
        _ = copy_throttled(client_read, upstream_write, upload_bytes_per_sec) => {}
        _ = tokio::io::copy(&mut upstream_read, &mut client_write) => {}
        _ = dropped.changed() => {}
    }
}

async fn copy_throttled(
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    bytes_per_sec: Arc<AtomicU64>,
) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        let read = from.read(&mut buf).await?;
        if read == 0 {
            return Ok(());
        }
        to.write_all(&buf[..read]).await?;
        let rate = bytes_per_sec.load(Ordering::Relaxed);
        if rate > 0 {
            tokio::time::sleep(Duration::from_secs_f64(read as f64 / rate as f64)).await;
        }
    }
}

/// A mailbox client wired the way the app wires one: pushing blobs through the
/// node's persisted push queue.
pub fn app_mailbox_client(
    node: &TestNode,
    mailbox_id: &mailbox_client::MailboxId,
    url: &str,
) -> ToyMailboxClient<MailboxOperation> {
    ToyMailboxClient::new(
        mailbox_id.clone(),
        url,
        node.endpoint_id(),
        node.blob_push_queue(),
    )
}

/// A client for reading what a mailbox holds, independent of any node.
pub fn inspection_client(
    mailbox_id: &mailbox_client::MailboxId,
    url: &str,
) -> ToyMailboxClient<MailboxOperation> {
    ToyMailboxClient::new(
        mailbox_id.clone(),
        url,
        iroh::SecretKey::generate().public(),
        Arc::new(mailbox_client::NoopBlobPushQueue),
    )
}
