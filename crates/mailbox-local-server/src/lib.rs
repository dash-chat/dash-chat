//! Spawn an in-process mailbox server that shares a node's iroh endpoint and
//! blob store, and announce it on the LAN via mDNS so peers can discover and
//! sync against it without any cloud service.

use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
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
    announcement: LocalHubAnnouncementService,
}

impl LocalMailboxServer {
    /// Retires the announcement, so peers hear this hub leave instead of
    /// waiting out its silence.
    pub async fn stop(self) {
        let _ = self.stop_signal.send(());
        let (_, served) = tokio::join!(self.announcement.shutdown(), self.task);
        if let Err(err) = served {
            log::error!("Local mailbox server task ended unexpectedly: {err}");
        }
    }
}

/// Spawn an in-process mailbox server sharing the given iroh endpoint and blob
/// store, so it serves blobs from the same store over the same endpoint. The
/// server is announced on the LAN over mDNS, owned by the returned server so
/// stopping it retires the announcement.
///
/// The port is remembered beside `db_path` and reused across restarts, so
/// peers that discovered this hub keep reaching it.
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
    let port = reserve_port(&db_path)?;
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

    let addr = serve_addr(port);
    let task = tokio::spawn(async move {
        let signal = async move {
            let _ = stop_signal_rx.await;
        };
        if let Err(e) = mailbox_server::spawn_server(
            db_path,
            addr,
            None,
            Some(blob_sync),
            None,
            *dashchat_utils::NETWORK_ID,
            signal,
        )
        .await
        {
            log::error!("Local mailbox server failed: {e:?}");
        }
    });

    let announcement = spawn_local_hub_announcement(endpoint_id, port)?;

    Ok(LocalMailboxServer {
        url: format!("http://127.0.0.1:{port}"),
        port,
        stop_signal,
        task,
        announcement,
    })
}

/// Dual-stack, so peers reach us over both the IPv4 and IPv6 addresses mDNS
/// announces: a `::` socket accepts IPv4 connections as v4-mapped addresses
/// where `IPV6_V6ONLY` defaults off (macOS, Linux).
fn serve_addr(port: u16) -> String {
    format!("[::]:{port}")
}

fn port_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("port")
}

/// The port to serve on: the one this hub served on last, when it is still
/// free, so a restart does not strand every peer that discovered it at the old
/// one. Falls back to any free port, and remembers whichever it took.
fn reserve_port(db_path: &Path) -> anyhow::Result<u16> {
    let remembered = fs::read_to_string(port_path(db_path))
        .ok()
        .and_then(|port| port.trim().parse::<u16>().ok());
    let port = match remembered {
        Some(port) => match std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            Ok(listener) => listener.local_addr()?.port(),
            Err(err) => {
                log::info!("Local mailbox port {port} is not free ({err}); taking another");
                any_free_port()?
            }
        },
        None => any_free_port()?,
    };
    if remembered != Some(port) {
        if let Err(err) = fs::write(port_path(db_path), port.to_string()) {
            log::warn!("Failed to remember local mailbox port {port}: {err}");
        }
    }
    Ok(port)
}

fn any_free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(listener.local_addr()?.port())
}

/// Announce a mailbox on the LAN so peers discover it. The instance id is the
/// hub's MailboxId (base64url-no-pad of the endpoint's public key). `port` is
/// where the server listens on every interface.
pub fn spawn_local_hub_announcement(
    endpoint_id: EndpointId,
    port: u16,
) -> anyhow::Result<LocalHubAnnouncementService> {
    LocalHubAnnouncementService::spawn(&encode_mailbox_id(endpoint_id), port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("mailbox.redb")
    }

    #[test]
    fn a_hub_serves_on_the_port_it_served_on_last() {
        let dir = tempfile::tempdir().unwrap();
        let first = reserve_port(&db_path(&dir)).unwrap();
        assert_eq!(reserve_port(&db_path(&dir)).unwrap(), first);
    }

    #[test]
    fn a_remembered_port_that_is_no_longer_free_is_replaced_and_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let first = reserve_port(&db_path(&dir)).unwrap();

        let taken = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, first)).unwrap();
        let second = reserve_port(&db_path(&dir)).unwrap();
        assert_ne!(second, first);

        // The new port is the one remembered from here on, whether or not the
        // old one comes free again.
        drop(taken);
        assert_eq!(reserve_port(&db_path(&dir)).unwrap(), second);
    }

    #[test]
    fn a_hub_with_nothing_remembered_takes_any_free_port() {
        let dir = tempfile::tempdir().unwrap();
        assert!(reserve_port(&db_path(&dir)).is_ok());
        assert!(port_path(&db_path(&dir)).exists());
    }
}
