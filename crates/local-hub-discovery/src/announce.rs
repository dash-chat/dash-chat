//! Advertising a local hub on the LAN over mDNS (swarm-discovery), so browsers
//! on the same network can discover it.

use std::net::{IpAddr, SocketAddr};

use swarm_discovery::DropGuard;
use tokio::sync::broadcast;
use tokio_util::task::AbortOnDropHandle;

use crate::{base_discoverer, SERVICE_NAME};

pub struct LocalHubAnnouncementService {
    // Holds the live announcement and re-arms it on each network change; drop to stop.
    _task: AbortOnDropHandle<()>,
}

impl LocalHubAnnouncementService {
    /// Announce a local hub on the LAN over mDNS (swarm-discovery), so browsers on
    /// the same network discover it. `instance_id` is the swarm id peers see.
    /// `bind_addr` is where the hub listens: an unspecified host (`[::]` / `0.0.0.0`)
    /// advertises every routable local IPv4 plus loopback; a specific host advertises
    /// just that one. Must be called within a Tokio runtime.
    pub fn spawn(instance_id: &str, bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let handle = tokio::runtime::Handle::current();
        // Eager first announce so a bad runtime or bind fails fast.
        let initial = announce(instance_id, bind_addr, &handle)?;
        let instance_id = instance_id.to_string();
        // swarm-discovery pins its multicast socket and advertised addresses at
        // spawn, so re-announce on every network change — as the browser re-binds.
        let task = AbortOnDropHandle::new(tokio::spawn(async move {
            let mut _announcement = Some(initial);
            let mut network = network_watch::network_change();
            while matches!(
                network.recv().await,
                Ok(()) | Err(broadcast::error::RecvError::Lagged(_))
            ) {
                _announcement = announce(&instance_id, bind_addr, &handle).ok();
            }
        }));
        Ok(Self { _task: task })
    }
}

/// Spawn a swarm-discovery announcer for a hub bound to `bind_addr`.
fn announce(
    instance_id: &str,
    bind_addr: SocketAddr,
    handle: &tokio::runtime::Handle,
) -> anyhow::Result<DropGuard> {
    let ips = announce_ips(bind_addr);
    log::info!(
        "Announcing local hub {instance_id} on the LAN via swarm-discovery ({SERVICE_NAME}) at {ips:?}:{}",
        bind_addr.port()
    );
    let guard = base_discoverer(instance_id)
        .with_addrs(bind_addr.port(), ips)
        .spawn(handle)?;
    Ok(guard)
}

/// The IPv4 addresses to advertise for a hub bound to `bind_addr`. mDNS is
/// IPv4-only here (see [`crate::base_discoverer`]), so IPv6 is not advertised;
/// link-local (169.254/16) is skipped to match [`crate::multicast_interfaces_v4`];
/// loopback is kept so a browser on the same host finds an in-process hub.
fn announce_ips(bind_addr: SocketAddr) -> Vec<IpAddr> {
    if !bind_addr.ip().is_unspecified() {
        return vec![bind_addr.ip()];
    }
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter_map(|interface| match interface.ip() {
                    IpAddr::V4(v4) if !v4.is_link_local() => Some(IpAddr::V4(v4)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
