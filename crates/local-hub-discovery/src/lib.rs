//! Local message hub discovery over mDNS (swarm-discovery), both halves:

use std::net::{IpAddr, Ipv4Addr};
use std::sync::LazyLock;

use data_encoding::{BASE32_NOPAD, BASE64URL_NOPAD};
use if_addrs::Interface;
use swarm_discovery::{Discoverer, IpClass};

mod announce;
mod discovery;

pub use announce::LocalHubAnnouncementService;
pub use discovery::{DiscoveredHub, LocalHubDiscoveryService};

/// The swarm-discovery service name Dash Chat hubs announce and browse under
/// (`_dashchat._tcp.local.` on the wire). Both halves must use the same name.
/// An e2e build is given an `E2E_NETWORK_ID`, a prefix of which suffixes the
/// name so that test runs sharing a LAN, each built with its own id, never see
/// each other's hubs. The whole id would overflow the 63-octet DNS label.
pub fn service_name() -> &'static str {
    static NAME: LazyLock<String> = LazyLock::new(|| {
        // A test announces under a namespace of its own: the production name is
        // browsed by real clients on whatever LAN the machine running it is on,
        // and they would discover, probe and poll the hub it spawns.
        if let Ok(id) = std::env::var("LOCAL_HUB_SERVICE_ID") {
            return format!("dashchat-{id}");
        }
        match option_env!("E2E_NETWORK_ID") {
            Some(id) => format!("dashchat-{}", &id[..8]),
            None => "dashchat".to_string(),
        }
    });
    &NAME
}

/// The TXT attribute a hub sets on its way out, so browsers retire it at once
/// instead of waiting for its announcements to lapse.
pub(crate) const GOODBYE_ATTRIBUTE: &str = "bye";

/// A `Discoverer` configured the way both halves need it: IPv4-only, with
/// multicast egress pinned to `interfaces`.
pub(crate) fn base_discoverer(instance_id: &str, interfaces: Vec<Ipv4Addr>) -> Discoverer {
    Discoverer::new_interactive(service_name().to_string(), instance_id.to_string())
        .with_ip_class(IpClass::V4Only)
        .with_protocol(swarm_discovery::Protocol::Tcp)
        .with_multicast_interfaces_v4(interfaces)
}

/// swarm-discovery announces a hub under its instance id as a DNS label, but a
/// MailboxId is base64url and a leading `-` from that alphabet is an invalid
/// label swarm-discovery refuses. Re-encode the key bytes as base32 (`A-Z2-7`
/// only, always a valid label and 52 chars < the 63-octet limit) for the wire;
/// [`label_to_mailbox_id`] reverses it on the browse side.
pub(crate) fn mailbox_id_to_label(mailbox_id: &str) -> anyhow::Result<String> {
    Ok(BASE32_NOPAD.encode(&BASE64URL_NOPAD.decode(mailbox_id.as_bytes())?))
}

/// Recover the MailboxId from a label produced by [`mailbox_id_to_label`].
/// DNS names are case-insensitive and mDNS delivers the label lowercased,
/// which base32 decoding does not accept as is.
pub(crate) fn label_to_mailbox_id(label: &str) -> anyhow::Result<String> {
    let label = label.to_ascii_uppercase();
    Ok(BASE64URL_NOPAD.encode(&BASE32_NOPAD.decode(label.as_bytes())?))
}

/// The local IPv4 interfaces to pin mDNS multicast egress to, and the pool the
/// announce side advertises from.
pub(crate) fn multicast_interfaces_v4() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .map(|interfaces| interfaces.iter().filter_map(mdns_address_v4).collect())
        .unwrap_or_default()
}

/// The address to pin multicast to and advertise for `interface`, if it can
/// carry mDNS at all
fn mdns_address_v4(interface: &Interface) -> Option<Ipv4Addr> {
    if interface.is_p2p() {
        return None;
    }
    match interface.ip() {
        IpAddr::V4(v4) if !v4.is_link_local() => Some(v4),
        _ => None,
    }
}

/// The IPv4 subnets this host is on, as `(address, prefix length)`. Loopback
/// is left out: a hub reached over it is on this host, not on a LAN.
pub(crate) fn local_subnets_v4() -> Vec<(Ipv4Addr, u8)> {
    if_addrs::get_if_addrs()
        .map(|interfaces| interfaces.iter().filter_map(subnet_v4).collect())
        .unwrap_or_default()
}

fn subnet_v4(interface: &Interface) -> Option<(Ipv4Addr, u8)> {
    match &interface.addr {
        if_addrs::IfAddr::V4(v4) if !v4.ip.is_loopback() && !v4.ip.is_link_local() => {
            Some((v4.ip, v4.prefixlen))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use if_addrs::{IfAddr, IfOperStatus, Ifv4Addr};

    use super::*;

    fn interface(name: &str, ip: Ipv4Addr, prefixlen: u8, is_p2p: bool) -> Interface {
        Interface {
            name: name.to_string(),
            addr: IfAddr::V4(Ifv4Addr {
                ip,
                netmask: Ipv4Addr::from(u32::MAX << (32 - prefixlen)),
                prefixlen,
                broadcast: None,
            }),
            index: None,
            oper_status: IfOperStatus::Up,
            is_p2p,
            #[cfg(windows)]
            adapter_name: String::new(),
        }
    }

    #[test]
    fn an_interfaces_address_is_where_a_peer_on_the_lan_reaches_us() {
        let wifi = interface("wlo1", Ipv4Addr::new(192, 168, 0, 105), 24, false);

        assert_eq!(
            mdns_address_v4(&wifi),
            Some(Ipv4Addr::new(192, 168, 0, 105))
        );
    }

    #[test]
    fn a_point_to_point_interface_is_not_announced() {
        let tunnel = interface("wg0", Ipv4Addr::new(10, 8, 0, 2), 32, true);

        assert_eq!(mdns_address_v4(&tunnel), None);
    }

    #[test]
    fn a_link_local_address_is_not_announced() {
        let unconfigured = interface("wlo1", Ipv4Addr::new(169, 254, 3, 4), 16, false);

        assert_eq!(mdns_address_v4(&unconfigured), None);
    }

    #[test]
    fn loopback_is_kept_for_the_host_that_has_nothing_else() {
        let loopback = interface("lo", Ipv4Addr::LOCALHOST, 8, false);

        assert_eq!(mdns_address_v4(&loopback), Some(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn loopback_and_link_local_are_not_subnets_a_hub_can_be_on() {
        assert_eq!(
            subnet_v4(&interface("lo", Ipv4Addr::LOCALHOST, 8, false)),
            None
        );
        assert_eq!(
            subnet_v4(&interface("wlo1", Ipv4Addr::new(169, 254, 3, 4), 16, false)),
            None
        );
        assert_eq!(
            subnet_v4(&interface(
                "wlo1",
                Ipv4Addr::new(192, 168, 0, 105),
                24,
                false
            )),
            Some((Ipv4Addr::new(192, 168, 0, 105), 24))
        );
    }

    #[test]
    fn a_mailbox_id_starting_with_a_hyphen_round_trips_through_a_valid_label() {
        // A key whose leading 6 bits are 0b111110 encodes to a base64url string
        // starting with '-' — an invalid DNS label, the case that crashed the
        // announce before this codec.
        let mailbox_id = BASE64URL_NOPAD.encode(&[0xF8; 32]);
        assert!(mailbox_id.starts_with('-'));

        let label = mailbox_id_to_label(&mailbox_id).unwrap();
        assert!(label
            .bytes()
            .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)));
        assert_eq!(label_to_mailbox_id(&label).unwrap(), mailbox_id);
    }

    #[test]
    fn a_label_lowercased_on_the_wire_still_recovers_the_mailbox_id() {
        let mailbox_id = BASE64URL_NOPAD.encode(&[0x5A; 32]);
        let label = mailbox_id_to_label(&mailbox_id).unwrap();
        assert_eq!(
            label_to_mailbox_id(&label.to_ascii_lowercase()).unwrap(),
            mailbox_id
        );
    }
}
