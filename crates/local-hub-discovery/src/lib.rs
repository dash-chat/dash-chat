//! Local message hub discovery over mDNS (swarm-discovery), both halves:

use std::net::{IpAddr, Ipv4Addr};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use data_encoding::BASE32_NOPAD;
use swarm_discovery::{Discoverer, IpClass};

mod announce;
mod discovery;

pub use announce::LocalHubAnnouncementService;
pub use discovery::{LocalHubDiscoveryService, LocalHubEvent};

/// The swarm-discovery service name Dash Chat hubs announce and browse under
/// (`_dashchat._tcp.local.` on the wire). Both halves must use the same name.
pub const SERVICE_NAME: &str = "dashchat";

/// A `Discoverer` configured the way both halves need it: IPv4-only, with
/// multicast egress pinned to `interfaces`.
pub(crate) fn base_discoverer(instance_id: &str, interfaces: Vec<Ipv4Addr>) -> Discoverer {
    Discoverer::new_interactive(SERVICE_NAME.to_string(), instance_id.to_string())
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
    Ok(BASE32_NOPAD.encode(&URL_SAFE_NO_PAD.decode(mailbox_id)?))
}

/// Recover the MailboxId from a label produced by [`mailbox_id_to_label`].
/// DNS names are case-insensitive and mDNS delivers the label lowercased,
/// which base32 decoding does not accept as is.
pub(crate) fn label_to_mailbox_id(label: &str) -> anyhow::Result<String> {
    let label = label.to_ascii_uppercase();
    Ok(URL_SAFE_NO_PAD.encode(BASE32_NOPAD.decode(label.as_bytes())?))
}

/// The local IPv4 interfaces to pin mDNS multicast egress to. Link-local
/// (169.254/16) is skipped: it can't route multicast reliably. Loopback is
/// kept: on a host with no other interface it is where its own hub is announced
/// and the only way to hear it.
pub(crate) fn multicast_interfaces_v4() -> Vec<Ipv4Addr> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mailbox_id_starting_with_a_hyphen_round_trips_through_a_valid_label() {
        // A key whose leading 6 bits are 0b111110 encodes to a base64url string
        // starting with '-' — an invalid DNS label, the case that crashed the
        // announce before this codec.
        let mailbox_id = URL_SAFE_NO_PAD.encode([0xF8; 32]);
        assert!(mailbox_id.starts_with('-'));

        let label = mailbox_id_to_label(&mailbox_id).unwrap();
        assert!(label
            .bytes()
            .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)));
        assert_eq!(label_to_mailbox_id(&label).unwrap(), mailbox_id);
    }

    #[test]
    fn a_label_lowercased_on_the_wire_still_recovers_the_mailbox_id() {
        let mailbox_id = URL_SAFE_NO_PAD.encode([0x5A; 32]);
        let label = mailbox_id_to_label(&mailbox_id).unwrap();
        assert_eq!(
            label_to_mailbox_id(&label.to_ascii_lowercase()).unwrap(),
            mailbox_id
        );
    }
}
