//! Spawn an in-process mailbox server that shares a node's iroh endpoint and
//! blob store, and announce it on the LAN via mDNS so peers can discover and
//! sync against it without any cloud service.

use std::net::{Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use iroh::EndpointId;
use iroh_blobs::api::downloader::Downloader;
use iroh_blobs::BlobsProtocol;
use mailbox_server::{encode_mailbox_id, BlobSync, FetchConfig};
use tokio::sync::mpsc::UnboundedSender;

pub use local_hub_discovery::LocalHubAnnouncementService;

/// A running in-process mailbox server. Call [`LocalMailboxServer::stop`] to
/// shut it down gracefully.
pub struct LocalMailboxServer {
    /// A loopback URL the server can be reached at locally (e.g. for health
    /// checks).
    pub url: String,
    pub port: u16,
    stop_signal: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
    // Held for its `Drop`, which retires the mDNS announcement.
    _announcement: LocalHubAnnouncementService,
}

impl LocalMailboxServer {
    pub async fn stop(self) {
        let _ = self.stop_signal.send(());
        if let Err(err) = self.task.await {
            log::error!("Local mailbox server task ended unexpectedly: {err}");
        }
    }
}

/// Spawn an in-process mailbox server sharing the given iroh endpoint and blob
/// store, so it serves blobs from the same store over the same endpoint. A free
/// port is allocated automatically; the server is announced on the LAN over mDNS,
/// owned by the returned server so stopping it retires the announcement.
///
/// `upload_grace` overrides how long the mailbox defers dialing a blob's source
/// after an announce that expects an inline upload; `None` uses the production
/// default.
pub async fn spawn_local_mailbox_server(
    db_path: PathBuf,
    blobs: BlobsProtocol,
    downloader: Downloader,
    endpoint: iroh::Endpoint,
    fetch_config: Option<FetchConfig>,
    upload_grace: Option<Duration>,
    peer_addr_tx: UnboundedSender<iroh::EndpointAddr>,
) -> anyhow::Result<LocalMailboxServer> {
    let port = free_port()?;
    // Captured before `endpoint` is moved into the blob sync.
    let endpoint_id = endpoint.id();

    let mut blob_sync = BlobSync::shared(blobs, downloader, endpoint, peer_addr_tx);
    if let Some(fetch_config) = fetch_config {
        blob_sync = blob_sync.with_fetch_config(fetch_config);
    }
    if let Some(upload_grace) = upload_grace {
        blob_sync = blob_sync.with_upload_grace(upload_grace);
    }

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel::<()>();

    // Bind dual-stack so peers can reach us over both the IPv4 and IPv6
    // addresses mDNS announces. A `::` socket accepts IPv4 connections as
    // v4-mapped addresses on platforms where `IPV6_V6ONLY` defaults off
    // (macOS, Linux).
    let addr = format!("[::]:{port}");
    let task = tokio::spawn(async move {
        let signal = async move {
            let _ = stop_signal_rx.await;
        };
        if let Err(e) =
            mailbox_server::spawn_server(db_path, addr, None, Some(blob_sync), None, signal).await
        {
            log::error!("Local mailbox server failed: {e:?}");
        }
    });

    let bind_addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, port));
    let announcement = spawn_local_hub_announcement(endpoint_id, bind_addr)?;

    Ok(LocalMailboxServer {
        url: format!("http://127.0.0.1:{port}"),
        port,
        stop_signal,
        task,
        _announcement: announcement,
    })
}

fn free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Announce a mailbox on the LAN so peers discover it. The instance id is the
/// hub's MailboxId (base64url-no-pad of the endpoint's public key). `bind_addr`
/// is where the server listens.
pub fn spawn_local_hub_announcement(
    endpoint_id: EndpointId,
    bind_addr: SocketAddr,
) -> anyhow::Result<LocalHubAnnouncementService> {
    local_hub_discovery::spawn_local_hub_announcement(&encode_mailbox_id(endpoint_id), bind_addr)
}
