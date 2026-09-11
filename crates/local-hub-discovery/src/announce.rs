//! Advertising a local hub on the LAN over mDNS (swarm-discovery), so browsers
//! on the same network can discover it.

use std::net::{IpAddr, Ipv4Addr};

use swarm_discovery::DropGuard;
use tokio::sync::broadcast;
use tokio_util::task::AbortOnDropHandle;

use crate::{base_discoverer, mailbox_id_to_label, multicast_interfaces_v4, SERVICE_NAME};

pub struct LocalHubAnnouncementService {
    // Holds the live announcement and re-arms it on each network change; drop to stop.
    _task: AbortOnDropHandle<()>,
}

impl LocalHubAnnouncementService {
    /// Announce a local hub on the LAN over mDNS (swarm-discovery), so browsers on
    /// the same network discover it. `instance_id` is the swarm id peers see;
    /// `port` is where the hub listens on every interface. Every routable local
    /// IPv4 is advertised (loopback only if there is none), re-enumerated on each
    /// network change. Must be called within a Tokio runtime.
    pub fn spawn(instance_id: &str, port: u16) -> anyhow::Result<Self> {
        let handle = tokio::runtime::Handle::current();
        // Eager first announce so a bad runtime or bind fails fast.
        let initial = announce(instance_id, port, &handle)?;
        let instance_id = instance_id.to_string();
        // swarm-discovery pins its multicast socket and advertised addresses at
        // spawn, so re-announce on every network change — as the browser re-binds.
        let task = AbortOnDropHandle::new(tokio::spawn(async move {
            let mut _announcement = initial;
            let mut network = network_watch::network_change();
            loop {
                match network.recv().await {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {
                        log::warn!(
                            "Network change signal closed; local hub {instance_id} keeps its current announcement"
                        );
                        return;
                    }
                }
                match announce(&instance_id, port, &handle) {
                    Ok(next) => _announcement = next,
                    Err(err) => log::warn!(
                        "Failed to re-announce local hub {instance_id} (keeping the previous announcement, retrying on next network change): {err}"
                    ),
                }
            }
        }));
        Ok(Self { _task: task })
    }
}

/// Spawn a swarm-discovery announcer for a hub listening on `port`.
fn announce(
    instance_id: &str,
    port: u16,
    handle: &tokio::runtime::Handle,
) -> anyhow::Result<DropGuard> {
    let interfaces = multicast_interfaces_v4();
    let ips = announce_ips(&interfaces);
    log::info!(
        "Announcing local hub {instance_id} on the LAN via swarm-discovery ({SERVICE_NAME}) at {ips:?}:{port}"
    );
    let guard = base_discoverer(&mailbox_id_to_label(instance_id)?, interfaces)
        .with_addrs(port, ips)
        .spawn(handle)?;
    Ok(guard)
}

/// The IPv4 addresses to advertise: the interfaces multicast goes out on
/// (mDNS is IPv4-only here, see [`crate::base_discoverer`]), minus loopback
/// whenever there is any other, since a browser elsewhere on the LAN would
/// take a 127.0.0.1 it hears to mean its own host.
fn announce_ips(interfaces: &[Ipv4Addr]) -> Vec<IpAddr> {
    let (loopback, routable): (Vec<Ipv4Addr>, Vec<Ipv4Addr>) =
        interfaces.iter().copied().partition(|v4| v4.is_loopback());
    let ips = if routable.is_empty() {
        loopback
    } else {
        routable
    };
    ips.into_iter().map(IpAddr::V4).collect()
}
