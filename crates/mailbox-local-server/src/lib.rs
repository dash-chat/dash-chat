//! Spawn an in-process mailbox server that shares a node's iroh endpoint and
//! blob store, and announce it on the LAN via mDNS so peers can discover and
//! sync against it without any cloud service.

use std::fs;
use std::net::Ipv6Addr;
use std::path::{Path, PathBuf};

use iroh::EndpointId;
use mailbox_server::encode_mailbox_id;

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
        // Goodbye first, while the server is still serving: a browser that hears
        // it retires the hub at once, where a refused probe deliberately means
        // nothing.
        self.announcement.shutdown().await;
        let _ = self.stop_signal.send(());
        let served = self.task.await;
        if let Err(err) = served {
            log::error!("Local mailbox server task ended unexpectedly: {err}");
        }
    }
}

/// Spawn an in-process mailbox server sharing the given iroh endpoint, so
/// blobs are pushed to and served from the store behind it. The server is
/// announced on the LAN over mDNS, owned by the returned server so stopping it
/// retires the announcement.
///
/// The port is remembered beside `db_path` and reused across restarts, so
/// peers that discovered this hub keep reaching it.
pub async fn spawn_local_mailbox_server(
    db_path: PathBuf,
    endpoint: iroh::Endpoint,
) -> anyhow::Result<LocalMailboxServer> {
    let (listener, port) = {
        let db_path = db_path.clone();
        tokio::task::spawn_blocking(move || reserve_listener(&db_path)).await??
    };
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    // Captured before `endpoint` is moved into the server task.
    let endpoint_id = endpoint.id();

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        let signal = async move {
            let _ = stop_signal_rx.await;
        };
        if let Err(e) = mailbox_server::spawn_server(
            db_path,
            listener,
            None,
            Some(endpoint),
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

fn port_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("port")
}

/// The socket to serve on, bound: on the port this hub served on last when it
/// is still free, so a restart does not strand every peer that discovered it at
/// the old one. Falls back to any free port, and remembers whichever it took.
///
/// Holding the socket is the reservation -- nothing can take the port between
/// here and the server taking it over.
pub fn reserve_listener(db_path: &Path) -> anyhow::Result<(std::net::TcpListener, u16)> {
    // A remembered port outlives its hub only as long as the store it belongs
    // to; next to a database that is gone it would pin an arbitrary port on a
    // fresh install.
    if !db_path.exists() {
        let _ = fs::remove_file(port_path(db_path));
    }
    let remembered = fs::read_to_string(port_path(db_path))
        .ok()
        .and_then(|port| port.trim().parse::<u16>().ok());
    let listener = match remembered {
        Some(port) => match std::net::TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)) {
            Ok(listener) => listener,
            Err(err) => {
                log::info!("Local mailbox port {port} is not free ({err}); taking another");
                any_free_listener()?
            }
        },
        None => any_free_listener()?,
    };
    let port = listener.local_addr()?.port();
    if remembered != Some(port) {
        if let Err(err) = fs::write(port_path(db_path), port.to_string()) {
            log::warn!("Failed to remember local mailbox port {port}: {err}");
        }
    }
    Ok((listener, port))
}

fn any_free_listener() -> anyhow::Result<std::net::TcpListener> {
    Ok(std::net::TcpListener::bind((Ipv6Addr::UNSPECIFIED, 0))?)
}

/// Announce a mailbox on the LAN so peers discover it. The instance id is the
/// hub's MailboxId (base64url-no-pad of the endpoint's public key). `port` is
/// where the server listens on every interface.
pub fn spawn_local_hub_announcement(
    endpoint_id: EndpointId,
    port: u16,
) -> anyhow::Result<LocalHubAnnouncementService> {
    LocalHubAnnouncementService::spawn(
        local_hub_discovery::service_name(),
        &encode_mailbox_id(endpoint_id),
        port,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store a hub is restarting on: a remembered port only counts next to
    /// the database it was remembered for.
    fn db_path(dir: &tempfile::TempDir) -> PathBuf {
        let db_path = dir.path().join("mailbox.redb");
        if !db_path.exists() {
            fs::write(&db_path, b"").unwrap();
        }
        db_path
    }

    fn remembered_port(db_path: &Path) -> Option<u16> {
        fs::read_to_string(port_path(db_path))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    #[test]
    fn a_hub_serves_on_the_port_it_served_on_last() {
        let dir = tempfile::tempdir().unwrap();
        let (listener, first) = reserve_listener(&db_path(&dir)).unwrap();
        drop(listener);
        let (_, again) = reserve_listener(&db_path(&dir)).unwrap();
        if again != first {
            // These run in parallel: another test can take the port in the
            // moment it is free between the two calls. The replacement is then
            // what is remembered, which is the case the next test covers.
            assert_eq!(remembered_port(&db_path(&dir)), Some(again));
            return;
        }
        assert_eq!(again, first);
    }

    #[test]
    fn a_remembered_port_that_is_no_longer_free_is_replaced_and_remembered() {
        let dir = tempfile::tempdir().unwrap();
        // Holding the first reservation is what makes its port unavailable.
        let (taken, first) = reserve_listener(&db_path(&dir)).unwrap();
        let (held, second) = reserve_listener(&db_path(&dir)).unwrap();
        assert_ne!(second, first);

        // The new port is the one remembered from here on, whether or not the
        // old one comes free again.
        drop(taken);
        drop(held);
        assert_eq!(reserve_listener(&db_path(&dir)).unwrap().1, second);
    }

    #[test]
    fn a_hub_with_nothing_remembered_takes_any_free_port() {
        let dir = tempfile::tempdir().unwrap();
        assert!(reserve_listener(&db_path(&dir)).is_ok());
        assert!(port_path(&db_path(&dir)).exists());
    }
}
