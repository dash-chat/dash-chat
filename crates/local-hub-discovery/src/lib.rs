//! Local message hub discovery over mDNS (swarm-discovery), both halves:

use std::net::{IpAddr, Ipv4Addr};

use swarm_discovery::{Discoverer, IpClass};

mod announce;
mod discovery;

pub use announce::{spawn_local_hub_announcement, LocalHubAnnouncementService};
pub use discovery::{LocalHubDiscoveryService, LocalHubEvent};

/// The swarm-discovery service name Dash Chat hubs announce and browse under
/// (`_dashchat._tcp.local.` on the wire). Both halves must use the same name.
pub const SERVICE_NAME: &str = "dashchat";

/// A `Discoverer` configured the way both halves need it: IPv4-only, with
/// multicast egress pinned to the current interfaces.
pub(crate) fn base_discoverer(instance_id: &str) -> Discoverer {
    Discoverer::new_interactive(SERVICE_NAME.to_string(), instance_id.to_string())
        .with_ip_class(IpClass::V4Only)
        .with_protocol(swarm_discovery::Protocol::Tcp)
        .with_multicast_interfaces_v4(multicast_interfaces_v4())
}

/// The local IPv4 interfaces to pin mDNS multicast egress to. Link-local
/// (169.254/16) is skipped: it can't route multicast reliably.
fn multicast_interfaces_v4() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter_map(|interface| match interface.ip() {
                    IpAddr::V4(v4) if !v4.is_link_local() => Some(v4),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
