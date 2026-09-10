//! Browsing for local hubs on the LAN over mDNS (swarm-discovery).
//!
//! swarm-discovery's synchronous callback forwards each sighting over a channel;
//! [`LocalHubDiscoveryService::recv`] TCP-probes the advertised addresses and
//! yields a [`LocalHubEvent`] once a hub is reachable (or has aged out). The
//! browser is respawned on every network change, since swarm-discovery joins the
//! multicast group only at spawn — a hub that becomes reachable after we gain a
//! routable IP is picked up on the next cadence, with no explicit retry here.

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use swarm_discovery::{DropGuard, Peer};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_util::task::AbortOnDropHandle;

use crate::{base_discoverer, SERVICE_NAME};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalHubEvent {
    /// A hub became reachable on the LAN
    /// `id` is the announcer's instance name (the hub's MailboxId); `url` is an
    /// `http://host:port` that accepted a TCP connection.
    Found { id: String, url: String },
    /// A hub aged out of the swarm.
    Lost { id: String },
}

/// A sighting handed from the swarm-discovery callback to [`recv`]: the hub's
/// instance name (its MailboxId) and the swarm-discovery snapshot of it (an
/// expired peer carries no addresses — see [`Peer::is_expiry`]).
type Sighting = (String, Peer);

pub struct LocalHubDiscoveryService {
    sightings: UnboundedReceiver<Sighting>,
    reachable: BTreeSet<String>,
    _browser: AbortOnDropHandle<()>,
}

impl LocalHubDiscoveryService {
    /// Browse for local hubs until the returned service is dropped.
    pub fn spawn() -> Self {
        let (sightings_tx, sightings_rx) = unbounded_channel::<Sighting>();
        let browser = spawn_browser(browse_id(), sightings_tx);
        log::info!("Started local hub discovery (swarm-discovery, {SERVICE_NAME})");
        Self {
            sightings: sightings_rx,
            reachable: BTreeSet::new(),
            _browser: browser,
        }
    }

    /// The next discovery event, or `None` once discovery has stopped. Probes
    /// each sighting and dedups by id, so a hub re-surfacing every cadence yields
    /// at most one `Found`.
    pub async fn recv(&mut self) -> Option<LocalHubEvent> {
        loop {
            let (id, peer) = self.sightings.recv().await?;
            if peer.is_expiry() {
                if self.reachable.remove(&id) {
                    return Some(LocalHubEvent::Lost { id });
                }
            } else if !self.reachable.contains(&id) {
                if let Some(url) = probe_reachable(&peer).await {
                    self.reachable.insert(id.clone());
                    return Some(LocalHubEvent::Found { id, url });
                }
            }
        }
    }
}

// Browse for hubs in the network, resilient to network changes
fn spawn_browser(browse_id: String, sightings: UnboundedSender<Sighting>) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(tokio::spawn(async move {
        // The live discoverer. Reassigning spawns the replacement before the
        // previous one drops, so a rebind never leaves a gap with no browser.
        let mut _discoverer = spawn_discoverer(&browse_id, sightings.clone());
        let mut network = network_watch::network_change();
        while matches!(
            network.recv().await,
            Ok(()) | Err(broadcast::error::RecvError::Lagged(_))
        ) {
            _discoverer = spawn_discoverer(&browse_id, sightings.clone());
        }
    }))
}

/// Spawn a swarm-discovery browser, or `None` if binding the multicast socket
/// fails (e.g. no routable IPv4 yet, mid network change) — the caller retries on
/// the next network change rather than treating it as fatal.
fn spawn_discoverer(browse_id: &str, sightings: UnboundedSender<Sighting>) -> Option<DropGuard> {
    let handle = tokio::runtime::Handle::current();
    let discoverer = base_discoverer(browse_id).with_callback(move |id, peer| {
        // Runs on swarm-discovery's thread and must not block, so it only
        // forwards the sighting; recv does the TCP probe.
        let _ = sightings.send((id.to_string(), peer.clone()));
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

/// A per-process browse id (a valid DNS label). swarm-discovery needs one even to
/// browse; ours only has to not collide with a hub's MailboxId.
fn browse_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("browse-{}-{}", std::process::id(), n)
}

/// TCP-probe a peer's advertised addresses
async fn probe_reachable(peer: &Peer) -> Option<String> {
    let (loopback, routable): (Vec<_>, Vec<_>) = peer
        .addrs()
        .iter()
        .copied()
        .partition(|(ip, _)| ip.is_loopback());
    for group in [routable, loopback] {
        if let Some(addr) = race_connect(&group).await {
            return Some(format!("http://{addr}"));
        }
    }
    None
}

/// The first of `addrs` to accept a connection, all raced together
async fn race_connect(addrs: &[(IpAddr, u16)]) -> Option<SocketAddr> {
    if addrs.is_empty() {
        return None;
    }
    let probes = addrs.iter().map(|&(ip, port)| {
        let addr = SocketAddr::from((ip, port));
        Box::pin(async move { probe_tcp(addr).await.then_some(addr).ok_or(()) })
    });
    futures::future::select_ok(probes)
        .await
        .ok()
        .map(|(addr, _)| addr)
}

async fn probe_tcp(addr: SocketAddr) -> bool {
    match tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect(addr)).await {
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
