//! Find local message hubs announced on the LAN over mDNS.
//!
//! The listening half of local-hub discovery; `mailbox-local-server` owns the
//! announcing half. This crate reports reachable hub URLs and nothing else — no
//! node, mailbox or app types — so it stays usable from anywhere in the
//! workspace and the caller decides what a discovered hub means.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use mdns_sd::{DaemonEvent, Receiver, ServiceDaemon, ServiceEvent};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// The mDNS service type Dash Chat hubs announce and browse.
pub const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";

/// The service type e2e builds use instead of [`MDNS_SERVICE_TYPE`].
pub const E2E_MDNS_SERVICE_TYPE: &str = "_dashchat-e2e._tcp.local.";

/// How long to go without a query. Kept short even while a hub is already known:
/// a hub that dies without a goodbye packet leaves its records cached, so
/// knowing about one is no evidence that one is still reachable.
const REBROWSE_INTERVAL: Duration = Duration::from_secs(15);

/// Per-address TCP probe budget.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// A hub that answered on the LAN, with an address already proven reachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredHub {
    /// The announcer-chosen instance name, stable across re-announcements.
    pub id: String,
    /// An `http://host:port` URL that accepted a TCP connection.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalHubEvent {
    /// A hub was resolved and found reachable. Re-emitted whenever it
    /// re-announces, so callers must treat registration as idempotent.
    Found(DiscoveredHub),
    /// A hub's records went away. Carries the same id as the [`Self::Found`].
    Lost { id: String },
}

/// A running browse. Dropping it stops discovery.
pub struct LocalHubDiscoveryService {
    events: UnboundedReceiver<LocalHubEvent>,
    task: tokio::task::JoinHandle<()>,
}

impl LocalHubDiscoveryService {
    /// The next discovery event, or `None` once the browse has stopped.
    pub async fn recv(&mut self) -> Option<LocalHubEvent> {
        self.events.recv().await
    }
}

impl Drop for LocalHubDiscoveryService {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Browse `service_type` for local hubs until the returned service is dropped.
pub fn spawn_local_hub_discovery(
    mdns: ServiceDaemon,
    service_type: &str,
) -> anyhow::Result<LocalHubDiscoveryService> {
    let receiver = rebrowse(&mdns, service_type)?;
    // Registered after the first browse, so the interfaces that already existed
    // don't arrive as changes and re-browse us immediately on startup.
    let monitor = mdns.monitor()?;
    log::info!("Started mdns browse for local hubs: {service_type}");

    let (events, events_rx) = unbounded_channel();
    let task = tokio::spawn(browse_loop(
        mdns,
        service_type.to_string(),
        receiver,
        monitor,
        events,
    ));

    Ok(LocalHubDiscoveryService {
        events: events_rx,
        task,
    })
}

/// Handle browse events, re-issuing the browse when the host gains an IP address
/// and, failing that, every [`REBROWSE_INTERVAL`].
async fn browse_loop(
    mdns: ServiceDaemon,
    service_type: String,
    first_receiver: Receiver<ServiceEvent>,
    monitor: Receiver<DaemonEvent>,
    events: UnboundedSender<LocalHubEvent>,
) {
    let mut receiver = first_receiver;
    loop {
        let reason = tokio::select! {
            () = handle_browse_events(&receiver, &events) => {
                // The handler only returns when the daemon closed the channel,
                // which a re-browse cannot recover from.
                return;
            }
            () = tokio::time::sleep(REBROWSE_INTERVAL) => "interval elapsed",
            () = wait_for_new_ip(&monitor) => "host gained an ip",
        };
        match rebrowse(&mdns, &service_type) {
            Ok(next) => {
                log::debug!("Re-issued mdns browse for local hubs ({reason})");
                receiver = next;
            }
            Err(err) => log::warn!("Failed to re-issue mdns browse: {err}"),
        }
    }
}

/// Restart the browse, resetting mdns-sd's re-query backoff. `stop_browse` is
/// what prunes the pending retransmissions, so skipping it stacks a new query
/// chain on every call.
fn rebrowse(mdns: &ServiceDaemon, service_type: &str) -> anyhow::Result<Receiver<ServiceEvent>> {
    mdns.stop_browse(service_type)?;
    Ok(mdns.browse(service_type)?)
}

/// Resolve once the daemon reports a newly acquired local IP address.
///
/// A network switch leaves the in-flight browse answered by nobody, and the one
/// query mdns-sd sends on a new interface is never retried — so a single dropped
/// packet would otherwise strand us until the interval fires. Never resolves if
/// the monitor channel is gone, which leaves the interval as the only trigger.
async fn wait_for_new_ip(monitor: &Receiver<DaemonEvent>) {
    loop {
        match monitor.recv_async().await {
            Ok(DaemonEvent::IpAdd(ip)) => {
                log::debug!("mdns daemon reported new local ip: {ip}");
                return;
            }
            Ok(_) => continue,
            Err(_) => std::future::pending::<()>().await,
        }
    }
}

async fn handle_browse_events(
    receiver: &Receiver<ServiceEvent>,
    events: &UnboundedSender<LocalHubEvent>,
) {
    while let Ok(event) = receiver.recv_async().await {
        match event {
            ServiceEvent::ServiceResolved(resolved) => {
                let id = instance_name_from_fullname(&resolved.fullname, &resolved.ty_domain);
                let port = resolved.port;
                let events = events.clone();
                // Probing runs off the handler so a slow or unreachable peer
                // cannot stall the event loop.
                tokio::spawn(async move {
                    let Some(host) = pick_reachable_host(&resolved, port).await else {
                        log::info!(
                            "Resolved mdns service {id}: no address in the announcement is reachable on port {port}, waiting for next announcement"
                        );
                        return;
                    };
                    let _ = events.send(LocalHubEvent::Found(DiscoveredHub {
                        id,
                        url: format!("http://{host}:{port}"),
                    }));
                });
            }
            ServiceEvent::ServiceRemoved(ty_domain, fullname) => {
                let id = instance_name_from_fullname(&fullname, &ty_domain);
                let _ = events.send(LocalHubEvent::Lost { id });
            }
            other_event => {
                log::trace!("((( Received other mdns event: {:?} )))", &other_event);
            }
        }
    }

    log::debug!("mdns browse handler loop ended");
}

/// Concurrently TCP-probe the routable addresses, then the loopback addresses,
/// returning the first that accepts a connection within `PROBE_TIMEOUT`,
/// formatted for a URL authority (IPv6 wrapped in `[...]`). Returns `None` if
/// no address in the announcement is reachable on the given port. The probe is
/// a bare TCP connect — it doesn't verify the listener is actually a mailbox
/// server, but in practice no other service shares the randomly-allocated port.
///
/// Non-loopback is tried first: it is the more stable identity in logs and works
/// regardless of which side announced. A loopback address from a *remote*
/// announcer would point at our own loopback, but the probe will fail unless we
/// happen to be listening on that exact port, which for a randomly-allocated
/// mailbox server port is vanishingly unlikely. For our *own* announcement, the
/// loopback fallback is what makes offline self-discovery work.
async fn pick_reachable_host(resolved: &mdns_sd::ResolvedService, port: u16) -> Option<String> {
    let ips = resolved.addresses.iter().filter_map(|addr| match addr {
        mdns_sd::ScopedIp::V4(ip) => Some(IpAddr::V4(*ip.addr())),
        // Link-local IPv6 (fe80::/10) needs a `%ifN` zone identifier to route;
        // the announcer's interface index isn't usable from here, so skip it.
        mdns_sd::ScopedIp::V6(ip) if !ip.addr().is_unicast_link_local() => {
            Some(IpAddr::V6(*ip.addr()))
        }
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
    let addr = SocketAddr::from((host, port));
    let connect = tokio::net::TcpStream::connect(&addr);
    match tokio::time::timeout(PROBE_TIMEOUT, connect).await {
        Ok(Ok(_stream)) => true,
        Ok(Err(err)) => {
            log::trace!("Probe to {addr} refused / errored: {err}");
            false
        }
        Err(_elapsed) => {
            log::trace!("Probe to {addr} timed out after {PROBE_TIMEOUT:?}");
            false
        }
    }
}

/// Recover the announcer-chosen instance name from a resolved mDNS fullname.
///
/// The fullname is shaped as `<instance>.<ty_domain>` (e.g.
/// `"f73abc...._dashchat._tcp.local."`). When the local mDNS daemon detects a
/// registration conflict (typically a stale cached entry from a previous
/// session), it appends a `" (N)"` uniqueness counter to the instance name —
/// so the fullname can come back as `"f73abc... (2)._dashchat._tcp.local."`.
/// Both the type suffix and the counter are mDNS plumbing the application
/// shouldn't see; strip them so the hub id stays the stable NODEID the
/// announcer set.
fn instance_name_from_fullname(fullname: &str, ty_domain: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_service_type_suffix() {
        assert_eq!(
            instance_name_from_fullname("nodeid._dashchat._tcp.local.", "_dashchat._tcp.local."),
            "nodeid"
        );
    }

    #[test]
    fn strips_a_bonjour_uniqueness_counter() {
        assert_eq!(
            instance_name_from_fullname(
                "nodeid (2)._dashchat._tcp.local.",
                "_dashchat._tcp.local."
            ),
            "nodeid"
        );
    }

    #[test]
    fn keeps_parenthesised_text_that_is_not_a_counter() {
        assert_eq!(strip_collision_suffix("nodeid (beta)"), "nodeid (beta)");
        assert_eq!(strip_collision_suffix("nodeid ()"), "nodeid ()");
    }

    #[test]
    fn wraps_ipv6_for_a_url_authority() {
        assert_eq!(url_host("192.168.1.5".parse().unwrap()), "192.168.1.5");
        assert_eq!(url_host("fd00::1".parse().unwrap()), "[fd00::1]");
    }
}
