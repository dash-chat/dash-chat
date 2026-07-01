//! Spawn an in-process mailbox server that shares a node's iroh endpoint and
//! blob store, and (optionally) announce it on the LAN via mDNS so peers can
//! discover and sync against it without any cloud service.
//!
//! This crate owns the `mdns-sd` / `if-watch` dependencies so that
//! `dashchat-node` does not have to: it consumes this crate only as a dev
//! dependency (for tests), while the Tauri host crate uses it in production.

use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use iroh::EndpointId;
use iroh_blobs::api::downloader::Downloader;
use iroh_blobs::BlobsProtocol;
use mailbox_server::{encode_mailbox_id, BlobSync, FetchConfig};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::sync::mpsc::UnboundedSender;

/// A running in-process mailbox server. Call [`LocalMailboxServer::stop`] to
/// shut it down gracefully.
pub struct LocalMailboxServer {
    /// A loopback URL the server can be reached at locally (e.g. for health
    /// checks). Peers on the LAN reach it via the mDNS-announced addresses.
    pub url: String,
    pub port: u16,
    stop_signal: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
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
/// store (the `BlobSync::shared` model), so relayed blobs land in the same store
/// served by the same endpoint and the mailbox's EndpointId equals
/// `endpoint_id`. A free port is allocated automatically.
///
/// The returned [`LocalMailboxServer`] is not yet announced on the LAN; pair it
/// with [`register_mdns_with_retry`] and [`spawn_interface_watcher`] for
/// discovery.
pub async fn spawn_local_mailbox_server(
    db_path: PathBuf,
    blobs: BlobsProtocol,
    downloader: Downloader,
    endpoint: iroh::Endpoint,
    fetch_config: Option<FetchConfig>,
    peer_addr_tx: UnboundedSender<iroh::EndpointAddr>,
) -> anyhow::Result<LocalMailboxServer> {
    let port = free_port()?;

    let mut blob_sync = BlobSync::shared(blobs, downloader, endpoint, peer_addr_tx);
    if let Some(fetch_config) = fetch_config {
        blob_sync = blob_sync.with_fetch_config(fetch_config);
    }

    let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel::<()>();

    // Bind dual-stack so peers can reach us over both the IPv4 and IPv6
    // addresses the mDNS record auto-announces. A `::` socket accepts IPv4
    // connections as v4-mapped addresses on platforms where `IPV6_V6ONLY`
    // defaults off (macOS, Linux).
    let addr = format!("[::]:{port}");
    let task = tokio::spawn(async move {
        let signal = async move {
            let _ = stop_signal_rx.await;
        };
        if let Err(e) =
            mailbox_server::spawn_server(db_path, addr, None, Some(blob_sync), signal).await
        {
            log::error!("Local mailbox server failed: {e:?}");
        }
    });

    Ok(LocalMailboxServer {
        url: format!("http://127.0.0.1:{port}"),
        port,
        stop_signal,
        task,
    })
}

fn free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Register the mailbox as an mDNS service, retrying up to `attempts` times.
/// Returns the registered service fullname (needed later to unregister).
pub fn register_mdns_with_retry(
    daemon: &ServiceDaemon,
    service_type: &str,
    endpoint_id: EndpointId,
    port: u16,
    attempts: u32,
) -> anyhow::Result<String> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        let service = mdns_service_info(service_type, endpoint_id, port)?;
        let fullname = service.get_fullname().to_string();
        log::info!(
            "Registering local mailbox service via mdns: {} ({})",
            fullname,
            service.get_type()
        );
        match daemon.register(service) {
            Ok(()) => return Ok(fullname),
            Err(e) => {
                log::error!(
                    "Failed to register local mailbox service via mdns, attempt {attempt} of {attempts}, error: {e:?}"
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("failed to register local mailbox service via mdns")))
}

/// Spawn a task that re-announces the mDNS record whenever a network interface
/// comes up or down. `enable_addr_auto()` only enumerates interfaces at
/// registration time, so without this the announcement misses interfaces (like
/// macOS Internet Sharing's bridge100) that appear after the mailbox starts.
///
/// `fullname` is shared with the caller so it always reflects the currently
/// registered service: the watcher overwrites it on every re-announce so a later
/// unregister targets the actually-registered service rather than a stale one.
pub fn spawn_interface_watcher(
    daemon: ServiceDaemon,
    service_type: String,
    endpoint_id: EndpointId,
    port: u16,
    fullname: Arc<StdMutex<String>>,
    stop: tokio::sync::oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run_interface_watcher(daemon, service_type, endpoint_id, port, fullname, stop).await;
    })
}

async fn run_interface_watcher(
    daemon: ServiceDaemon,
    service_type: String,
    endpoint_id: EndpointId,
    port: u16,
    fullname: Arc<StdMutex<String>>,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) {
    use futures::StreamExt;
    let mut watcher = match if_watch::tokio::IfWatcher::new() {
        Ok(w) => w,
        Err(err) => {
            log::warn!("Failed to start mailbox interface watcher: {err:?}");
            return;
        }
    };

    loop {
        tokio::select! {
            biased;
            _ = &mut stop => {
                log::debug!("Mailbox interface watcher stopped");
                return;
            }
            event = watcher.next() => {
                let Some(event) = event else {
                    log::warn!("Mailbox interface watcher stream ended");
                    return;
                };
                match event {
                    Ok(if_watch::IfEvent::Up(net)) => {
                        log::info!("Mailbox interface watcher: up {net}, re-announcing mDNS");
                    }
                    Ok(if_watch::IfEvent::Down(net)) => {
                        log::info!("Mailbox interface watcher: down {net}, re-announcing mDNS");
                    }
                    Err(err) => {
                        log::warn!("Mailbox interface watcher error: {err:?}");
                        continue;
                    }
                }

                if !debounce_reannounce_burst(&mut watcher, &mut stop).await {
                    return;
                }
                let prev = fullname.lock().unwrap_or_else(|p| p.into_inner()).clone();
                if let Some(new_fullname) = reannounce_mdns(&daemon, &service_type, &prev, endpoint_id, port) {
                    *fullname.lock().unwrap_or_else(|p| p.into_inner()) = new_fullname;
                }
            }
        }
    }
}

/// Wait 500ms after an interface event so a burst of related changes triggers
/// a single re-announce. Returns false if the stop signal fired during the
/// wait (caller should treat that as shutdown) or if the watcher stream ended.
async fn debounce_reannounce_burst(
    watcher: &mut if_watch::tokio::IfWatcher,
    stop: &mut tokio::sync::oneshot::Receiver<()>,
) -> bool {
    use futures::StreamExt;
    let debounce = tokio::time::sleep(std::time::Duration::from_millis(500));
    tokio::pin!(debounce);
    loop {
        tokio::select! {
            biased;
            _ = &mut *stop => return false,
            _ = &mut debounce => return true,
            next = watcher.next() => {
                if next.is_none() {
                    return false;
                }
            }
        }
    }
}

/// Returns the fullname that is now registered with the daemon, or `None` if
/// the re-register failed (in which case the previous `fullname` is no longer
/// registered either — the unregister already ran).
fn reannounce_mdns(
    daemon: &ServiceDaemon,
    service_type: &str,
    fullname: &str,
    endpoint_id: EndpointId,
    port: u16,
) -> Option<String> {
    let _ = daemon.unregister(fullname);
    let service = match mdns_service_info(service_type, endpoint_id, port) {
        Ok(service) => service,
        Err(err) => {
            log::warn!("Failed to build mDNS service info for re-announce: {err:?}");
            return None;
        }
    };
    let new_fullname = service.get_fullname().to_string();
    if let Err(err) = daemon.register(service) {
        log::warn!("Failed to re-register mailbox mDNS: {err:?}");
        return None;
    }
    Some(new_fullname)
}

fn mdns_service_info(
    service_type: &str,
    endpoint_id: EndpointId,
    port: u16,
) -> anyhow::Result<ServiceInfo> {
    // The base64url-no-pad MailboxId encoding of the endpoint's 32-byte public
    // key (43 chars, fits a single DNS label) is used as the instance name so
    // the mDNS instance name IS the canonical MailboxId.
    let instance_name = encode_mailbox_id(endpoint_id);

    // Per-device hostname so the A/AAAA owner-name doesn't collide with every
    // other Dash Chat instance on the LAN. A shared hostname can cause one
    // instance's address cache entry to overwrite another's in the resolver.
    let host_name = format!("{instance_name}.local.");

    Ok(
        ServiceInfo::new(service_type, &instance_name, &host_name, "", port, vec![])?
            .enable_addr_auto(),
    )
}
