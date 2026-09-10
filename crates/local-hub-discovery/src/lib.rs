//! Local message hub discovery over mDNS (swarm-discovery), both halves:
//! [`spawn_local_hub_discovery`] browses for hubs on the LAN,
//! [`spawn_local_hub_announcement`] advertises one.

use std::net::{IpAddr, Ipv4Addr};

use swarm_discovery::{Discoverer, IpClass};

mod announce;
mod discovery;

pub use announce::{spawn_local_hub_announcement, LocalHubAnnouncementService};
pub use discovery::{
    spawn_local_hub_discovery, DiscoveredHub, LocalHubDiscoveryService, LocalHubEvent,
};

/// The swarm-discovery service name Dash Chat hubs announce and browse under
/// (`_dashchat._udp.local.` on the wire). Both halves must use the same name.
pub const SERVICE_NAME: &str = "dashchat";

/// A `Discoverer` configured the way both halves need it: IPv4-only, with
/// multicast egress pinned to the current interfaces.
///
/// IPv4-only because swarm's IPv6 multicast always uses the default interface,
/// which on Android has no route to `ff02::fb` — every v6 send just fails
/// "Network is unreachable". Interface pinning because the wildcard socket's
/// default multicast route is also gone right after an Android Wi-Fi switch, so
/// sends fail there too until each live interface is pinned explicitly.
pub(crate) fn base_discoverer(instance_id: &str) -> Discoverer {
    Discoverer::new_interactive(SERVICE_NAME.to_string(), instance_id.to_string())
        .with_ip_class(IpClass::V4Only)
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
