//! Whether this device's mDNS multicast can leave it at all, which mDNS
//! libraries only log about. macOS refuses it with EHOSTUNREACH to an app
//! without Local Network access (TN3179), and so does a VPN that leaves a
//! reject route on 224.0.0.0/4: nobody on the LAN can discover the device.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use futures::Stream;
use if_addrs::Interface;
use socket2::{Domain, Protocol, Socket, Type};

/// How often to probe: on macOS, Local Network access can be granted or
/// revoked at any time.
const PROBE_INTERVAL: Duration = Duration::from_secs(3);

/// Rounds in a row every LAN must refuse before the LAN counts as blocked, so
/// a refusal during a network change raises no alarm.
const BLOCKED_ROUNDS: u32 = 2;

const MDNS_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);

/// A DNS header asking nothing, which mDNS responders ignore.
const EMPTY_QUERY: [u8; 12] = [0; 12];

/// Whether every LAN this device is on refuses its mDNS multicast
pub fn lan_multicast_blocked() -> impl Stream<Item = bool> {
    async_stream::stream! {
        let mut refused_rounds = 0;
        let mut last_verdict = None;
        loop {
            if every_lan_refuses_multicast().await {
                refused_rounds += 1;
            } else {
                refused_rounds = 0;
            }
            let blocked = refused_rounds >= BLOCKED_ROUNDS;
            if last_verdict != Some(blocked) {
                if last_verdict.is_some() {
                    log_change(blocked);
                }
                last_verdict = Some(blocked);
                yield blocked;
            }
            tokio::time::sleep(PROBE_INTERVAL).await;
        }
    }
}

/// The local IPv4 interfaces that can carry mDNS, loopback included.
pub fn multicast_interfaces_v4() -> Vec<Ipv4Addr> {
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

/// A device with no LAN is not blocked, only offline.
async fn every_lan_refuses_multicast() -> bool {
    let lans: Vec<Ipv4Addr> = multicast_interfaces_v4()
        .into_iter()
        .filter(|ip| !ip.is_loopback())
        .collect();
    if lans.is_empty() {
        return false;
    }
    // Socket calls block.
    tokio::task::spawn_blocking(move || lans.into_iter().all(|lan| send_probe(lan).is_err()))
        .await
        .unwrap_or(false)
}

fn log_change(blocked: bool) {
    if blocked {
        log::warn!("mDNS can't reach the LAN: multicast is refused");
    } else {
        log::info!("mDNS reaches the LAN again");
    }
}

fn send_probe(interface: Ipv4Addr) -> std::io::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_if_v4(&interface)?;
    socket.send_to(&EMPTY_QUERY, &MDNS_V4.into())?;
    Ok(())
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

    const WIFI: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 105);

    #[test]
    fn an_interfaces_address_is_where_a_peer_on_the_lan_reaches_us() {
        assert_eq!(
            mdns_address_v4(&interface("wlo1", WIFI, 24, false)),
            Some(WIFI)
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
}
