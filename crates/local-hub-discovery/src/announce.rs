//! Advertising a local hub on the LAN over mDNS (swarm-discovery), so browsers
//! on the same network can discover it.

use std::net::{IpAddr, SocketAddr};

use swarm_discovery::DropGuard;

use crate::{base_discoverer, SERVICE_NAME};

pub struct LocalHubAnnouncementService {
    // Its only job is to stay alive: `Drop` retires the swarm-discovery entry.
    _guard: DropGuard,
}

impl LocalHubAnnouncementService {
    /// Announce a local hub on the LAN over mDNS (swarm-discovery), so browsers on
    /// the same network discover it. `instance_id` is the swarm id peers see.
    /// `bind_addr` is where the hub listens: an unspecified host (`[::]` / `0.0.0.0`)
    /// advertises every routable local IPv4 plus loopback; a specific host advertises
    /// just that one. Must be called within a Tokio runtime.
    pub fn spawn(instance_id: &str, bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let handle = tokio::runtime::Handle::current();
        let ips = announce_ips(bind_addr);
        log::info!(
            "Announcing local hub {instance_id} on the LAN via swarm-discovery ({SERVICE_NAME}) at {ips:?}:{}",
            bind_addr.port()
        );
        let guard = base_discoverer(instance_id)
            .with_addrs(bind_addr.port(), ips)
            .spawn(&handle)?;
        Ok(Self { _guard: guard })
    }
}

/// The IPv4 addresses to advertise for a hub bound to `bind_addr`. mDNS is
/// IPv4-only here (see [`crate::base_discoverer`]), so IPv6 is not advertised;
/// loopback is kept so a browser on the same host finds an in-process hub.
fn announce_ips(bind_addr: SocketAddr) -> Vec<IpAddr> {
    if !bind_addr.ip().is_unspecified() {
        return vec![bind_addr.ip()];
    }
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .map(|interface| interface.ip())
                .filter(|ip| ip.is_ipv4())
                .collect()
        })
        .unwrap_or_default()
}
