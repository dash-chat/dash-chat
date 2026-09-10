//! Browsing for local hubs on the LAN over mDNS (swarm-discovery).
//!
//! swarm-discovery re-surfaces every announcer once per cadence, so a hub that
//! only becomes reachable after we gain a routable IP is picked up on the next
//! cadence without any explicit retry here.

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use swarm_discovery::DropGuard;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_util::task::AbortOnDropHandle;

use crate::{base_discoverer, SERVICE_NAME};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// A hub that answered on the LAN, with an address already proven reachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredHub {
    /// The announcer-chosen instance name (the hub's MailboxId).
    pub id: String,
    /// An `http://host:port` URL that accepted a TCP connection.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalHubEvent {
    /// Emitted once per hub, on the transition to reachable.
    Found(DiscoveredHub),
    /// A hub aged out of the swarm.
    Lost { id: String },
}

/// A running discovery; drop it to stop. It rebinds itself on every network
/// change.
pub struct LocalHubDiscoveryService {
    events: UnboundedReceiver<LocalHubEvent>,
    _rebinder: AbortOnDropHandle<()>,
    _prober: AbortOnDropHandle<()>,
}

impl LocalHubDiscoveryService {
    /// The next discovery event, or `None` once discovery has stopped.
    pub async fn recv(&mut self) -> Option<LocalHubEvent> {
        self.events.recv().await
    }
}

/// Browse for local hubs until the returned service is dropped.
pub fn spawn_local_hub_discovery() -> LocalHubDiscoveryService {
    let (obs_tx, obs_rx) = unbounded_channel::<Observation>();
    let (events, events_rx) = unbounded_channel::<LocalHubEvent>();

    let rebinder = spawn_rebind_on_network_change(browse_id(), obs_tx);
    let prober = spawn_reachability_prober(obs_rx, events);

    log::info!("Started local hub discovery (swarm-discovery, {SERVICE_NAME})");
    LocalHubDiscoveryService {
        events: events_rx,
        _rebinder: rebinder,
        _prober: prober,
    }
}

/// A hub sighting forwarded from swarm-discovery's synchronous callback to the
/// async prober.
struct Observation {
    id: String,
    ips: Vec<Ipv4Addr>,
    port: u16,
    expired: bool,
}

/// Spawn a swarm-discovery browser, or `None` if binding the multicast socket
/// fails (e.g. no routable IPv4 yet, mid network change) — the caller retries on
/// the next network change rather than treating it as fatal.
fn spawn_discoverer(browse_id: &str, obs_tx: UnboundedSender<Observation>) -> Option<DropGuard> {
    let handle = tokio::runtime::Handle::current();
    let discoverer = base_discoverer(browse_id).with_callback(move |id, peer| {
        // Runs on swarm-discovery's thread and must not block, so it only
        // forwards the sighting; the prober does the TCP probe.
        let addrs = peer.addrs();
        let port = addrs.first().map(|(_, port)| *port).unwrap_or_default();
        let ips = addrs
            .iter()
            .filter_map(|(ip, _)| match ip {
                IpAddr::V4(v4) => Some(*v4),
                IpAddr::V6(_) => None,
            })
            .collect();
        let _ = obs_tx.send(Observation {
            id: id.to_string(),
            ips,
            port,
            expired: peer.is_expiry(),
        });
    });
    match discoverer.spawn(&handle) {
        Ok(guard) => Some(guard),
        Err(err) => {
            log::warn!(
                "Failed to bind local hub discovery (retrying on next network change): {err}"
            );
            None
        }
    }
}

/// swarm-discovery joins the multicast group once at spawn and never rejoins, so
/// this respawns the discoverer whenever the network settles. The first spawn is
/// eager; every spawn (initial or rebind) is non-fatal — a transient bind failure
/// is retried on the next settle rather than propagated.
#[allow(unused_variables, unused_assignments)]
fn spawn_rebind_on_network_change(
    browse_id: String,
    obs_tx: UnboundedSender<Observation>,
) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(tokio::spawn(async move {
        // Held only for its `Drop` (stops the previous discoverer); reassigned on
        // each respawn, so it is written but never read.
        let mut guard = spawn_discoverer(&browse_id, obs_tx.clone());
        let mut network = network_watch::network_change();
        while matches!(
            network.recv().await,
            Ok(()) | Err(broadcast::error::RecvError::Lagged(_))
        ) {
            guard = spawn_discoverer(&browse_id, obs_tx.clone());
        }
    }))
}

fn spawn_reachability_prober(
    obs: UnboundedReceiver<Observation>,
    events: UnboundedSender<LocalHubEvent>,
) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(tokio::spawn(async move {
        let mut obs = obs;
        let mut reachable: BTreeSet<String> = BTreeSet::new();
        while let Some(sighting) = obs.recv().await {
            if sighting.expired {
                if reachable.remove(&sighting.id) {
                    let _ = events.send(LocalHubEvent::Lost { id: sighting.id });
                }
                continue;
            }
            if reachable.contains(&sighting.id) {
                continue;
            }
            if let Some(host) = probe_reachable(&sighting.ips, sighting.port).await {
                reachable.insert(sighting.id.clone());
                let _ = events.send(LocalHubEvent::Found(DiscoveredHub {
                    id: sighting.id,
                    url: format!("http://{host}:{}", sighting.port),
                }));
            }
        }
    }))
}

/// A per-process browse id (a valid DNS label). swarm-discovery needs one even to
/// browse; ours only has to not collide with a hub's MailboxId.
fn browse_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("browse-{}-{}", std::process::id(), n)
}

/// TCP-probe the routable addresses, then loopback, returning the first to accept
/// a connection within `PROBE_TIMEOUT`; `None` if none answer. A bare connect
/// doesn't confirm the listener is a mailbox, but nothing else uses the announced
/// port. Loopback is the fallback that lets a host find its own in-process hub
/// while offline.
async fn probe_reachable(ips: &[Ipv4Addr], port: u16) -> Option<String> {
    let (loopback, routable): (Vec<_>, Vec<_>) =
        ips.iter().copied().partition(|ip| ip.is_loopback());
    for group in [routable, loopback] {
        if group.is_empty() {
            continue;
        }
        let probes = group.iter().map(|&ip| {
            Box::pin(async move {
                if probe_tcp(ip, port).await {
                    Ok::<Ipv4Addr, ()>(ip)
                } else {
                    Err(())
                }
            })
        });
        if let Ok((ip, _)) = futures::future::select_ok(probes).await {
            return Some(ip.to_string());
        }
    }
    None
}

async fn probe_tcp(host: Ipv4Addr, port: u16) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_id_is_a_dns_label() {
        let id = browse_id();
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        assert!(id.len() <= 63);
    }
}
