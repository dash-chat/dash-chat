//! Browse for mailbox services on the LAN, TCP-probe the resolved addresses,
//! and register each reachable peer into a [`Mailboxes`] manager.

use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use mailbox_client::manager::Mailboxes;
use mailbox_client::store::MailboxStore;
use mailbox_client::toy::{ToyItemTraits, ToyMailboxClient};
use mailbox_client::{MailboxId, MailboxItem, OptionalItemTraits, UnfetchedBlobTracker};
use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent};

/// Browse the LAN for mailboxes and register each reachable peer directly into
/// `mailboxes` (as a [`ToyMailboxClient`] reachable at its URL), unregistering it
/// on removal; runs until cancelled. `our_endpoint` is our own iroh id, used as
/// the client's `sender_pubkey`; `tracker` receives the client's unfetched
/// blob-hash bookkeeping. `on_registered(mailbox_id, url)` runs after each
/// (re-)registration so callers can do their own per-mailbox setup (e.g. the
/// app adds the mailbox's dialing address to its address book and registers its
/// own address back). Re-issues the browse when network interfaces change (the
/// browse receiver is bound to the interface set present at `browse()` time).
pub async fn discover_mailboxes_loop<Item, Store, F, Fut>(
    daemon: ServiceDaemon,
    service_type: String,
    mailboxes: Mailboxes<Item, Store>,
    our_endpoint: iroh::EndpointId,
    tracker: Arc<dyn UnfetchedBlobTracker>,
    on_registered: F,
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: ToyItemTraits + OptionalItemTraits,
    Item::Author: ToyItemTraits,
    F: Fn(MailboxId, String) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut watcher = match if_watch::tokio::IfWatcher::new() {
        Ok(w) => w,
        Err(err) => {
            log::warn!("browse interface watcher failed to start: {err:?}");
            return;
        }
    };

    loop {
        match daemon.browse(&service_type) {
            Ok(receiver) => {
                log::info!("Started mDNS browse for {service_type}");
                tokio::select! {
                    _ = handle_browse_events(receiver, mailboxes.clone(), our_endpoint, tracker.clone(), on_registered.clone()) => {
                        log::warn!("mDNS browse event stream ended");
                        return;
                    }
                    changed = dashchat_utils::wait_for_interface_change(&mut watcher) => {
                        if !changed {
                            return;
                        }
                        if let Err(err) = daemon.stop_browse(&service_type) {
                            log::warn!("failed to stop mDNS browse during refresh: {err:?}");
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!("failed to start mDNS browse for {service_type}: {err:?}");
                if !dashchat_utils::wait_for_interface_change(&mut watcher).await {
                    return;
                }
            }
        }
    }
}

async fn handle_browse_events<Item, Store, F, Fut>(
    receiver: mdns_sd::Receiver<ServiceEvent>,
    mailboxes: Mailboxes<Item, Store>,
    our_endpoint: iroh::EndpointId,
    tracker: Arc<dyn UnfetchedBlobTracker>,
    on_registered: F,
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: ToyItemTraits + OptionalItemTraits,
    Item::Author: ToyItemTraits,
    F: Fn(MailboxId, String) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    while let Ok(event) = receiver.recv_async().await {
        match event {
            ServiceEvent::ServiceResolved(resolved) => {
                let mailbox_id =
                    instance_name_from_fullname(&resolved.fullname, &resolved.ty_domain);
                let port = resolved.port;
                let mailboxes = mailboxes.clone();
                let tracker = tracker.clone();
                let on_registered = on_registered.clone();
                // Probe + register off the event loop so a slow/unreachable peer
                // can't stall it.
                tokio::spawn(async move {
                    let Some(host) = pick_reachable_host(&resolved, port).await else {
                        log::info!("peer {mailbox_id}: no announced address reachable on :{port}");
                        return;
                    };
                    let url = format!("http://{host}:{port}");
                    mailboxes
                        .register(ToyMailboxClient::new(
                            mailbox_id.clone(),
                            url.clone(),
                            our_endpoint,
                            tracker,
                        ))
                        .await;
                    on_registered(mailbox_id, url).await;
                });
            }
            ServiceEvent::ServiceRemoved(ty_domain, fullname) => {
                let mailbox_id = instance_name_from_fullname(&fullname, &ty_domain);
                mailboxes.unregister(&mailbox_id).await;
            }
            _ => {}
        }
    }
}

/// Per-address TCP probe budget; generous enough for a sleepy mobile peer over
/// lossy Wi-Fi.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Concurrently TCP-probe routable addresses first, then loopback, returning the
/// first that accepts a connection, formatted for a URL authority.
async fn pick_reachable_host(resolved: &ResolvedService, port: u16) -> Option<String> {
    let ips = resolved.addresses.iter().filter_map(|addr| match addr {
        ScopedIp::V4(ip) => Some(IpAddr::V4(*ip.addr())),
        // Link-local IPv6 needs a zone id we can't supply from here; skip it.
        ScopedIp::V6(ip) if !ip.addr().is_unicast_link_local() => Some(IpAddr::V6(*ip.addr())),
        _ => None,
    });
    let (loopback, routable): (Vec<_>, Vec<_>) = ips.partition(|ip| ip.is_loopback());
    if let Some(host) = probe_first_reachable(&routable, port).await {
        return Some(host);
    }
    probe_first_reachable(&loopback, port).await
}

async fn probe_first_reachable(ips: &[IpAddr], port: u16) -> Option<String> {
    if ips.is_empty() {
        return None;
    }
    let probes = ips.iter().map(|&ip| {
        Box::pin(async move {
            if probe_tcp(ip, port).await {
                Ok::<IpAddr, ()>(ip)
            } else {
                Err(())
            }
        })
    });
    futures::future::select_ok(probes)
        .await
        .ok()
        .map(|(ip, _)| url_host(ip))
}

fn url_host(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    }
}

async fn probe_tcp(host: IpAddr, port: u16) -> bool {
    let addr = std::net::SocketAddr::from((host, port));
    match tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,
        Ok(Err(err)) => {
            log::trace!("probe to {addr} refused: {err}");
            false
        }
        Err(_) => {
            log::trace!("probe to {addr} timed out");
            false
        }
    }
}

/// Recover the announcer-chosen instance name (the MailboxId) from a resolved
/// mDNS fullname, stripping the service-type suffix and any `" (N)"` collision
/// counter an mDNS daemon may append.
pub fn instance_name_from_fullname(fullname: &str, ty_domain: &str) -> String {
    let instance = fullname
        .strip_suffix(ty_domain)
        .and_then(|s| s.strip_suffix('.'))
        .unwrap_or(fullname);
    strip_collision_suffix(instance).to_string()
}

fn strip_collision_suffix(name: &str) -> &str {
    let Some(stripped) = name.strip_suffix(')') else {
        return name;
    };
    let Some(open_idx) = stripped.rfind(" (") else {
        return name;
    };
    let inner = &stripped[open_idx + 2..];
    if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
        &name[..open_idx]
    } else {
        name
    }
}
