//! Whether this device's mDNS multicast can reach the LAN. macOS refuses it to
//! an app without Local Network access (TN3179), and so do VPN kill switches,
//! while mDNS libraries only log the error.

use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;

const PROBE_INTERVAL: Duration = Duration::from_secs(3);

/// More than one, so a refusal during a network change raises no alarm.
const REFUSED_ROUNDS_BEFORE_BLOCKED: u32 = 2;

const MDNS_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);

const EMPTY_DNS_HEADER: [u8; 12] = [0; 12];

pub struct LanMulticastProbe {
    blocked: watch::Receiver<bool>,
    _task: AbortOnDropHandle<()>,
}

impl LanMulticastProbe {
    pub fn spawn() -> Self {
        let (blocked_tx, blocked) = watch::channel(false);
        Self {
            blocked,
            _task: AbortOnDropHandle::new(tokio::spawn(probe_forever(blocked_tx))),
        }
    }

    pub fn blocked(&self) -> watch::Receiver<bool> {
        self.blocked.clone()
    }
}

async fn probe_forever(blocked_tx: watch::Sender<bool>) {
    let mut refused_rounds = 0;
    loop {
        match probe_every_lan().await {
            Some(true) => refused_rounds += 1,
            Some(false) => refused_rounds = 0,
            None => {}
        }
        let blocked = refused_rounds >= REFUSED_ROUNDS_BEFORE_BLOCKED;
        if *blocked_tx.borrow() != blocked {
            log_change(blocked);
            blocked_tx.send_replace(blocked);
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
}

async fn probe_every_lan() -> Option<bool> {
    let loopback = loopback_addresses();
    let mut probes = Vec::new();
    for lan in local_hub_discovery::multicast_interfaces_v4() {
        if !loopback.contains(&IpAddr::V4(lan)) {
            probes.push(refuses_multicast(lan).await);
        }
    }
    every_lan_refused(probes)
}

/// Every address on a loopback interface, including aliases like 10.10.10.1
/// on lo0 that aren't loopback addresses themselves.
fn loopback_addresses() -> HashSet<IpAddr> {
    let interfaces = if_addrs::get_if_addrs().unwrap_or_default();
    let loopback_names: HashSet<&str> = interfaces
        .iter()
        .filter(|interface| interface.is_loopback())
        .map(|interface| interface.name.as_str())
        .collect();
    interfaces
        .iter()
        .filter(|interface| loopback_names.contains(interface.name.as_str()))
        .map(|interface| interface.ip())
        .collect()
}

fn every_lan_refused(probes: Vec<io::Result<bool>>) -> Option<bool> {
    if probes.is_empty() || probes.iter().any(|probe| matches!(probe, Ok(false))) {
        Some(false)
    } else if probes.iter().any(|probe| probe.is_err()) {
        None
    } else {
        Some(true)
    }
}

async fn refuses_multicast(interface: Ipv4Addr) -> io::Result<bool> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_if_v4(&interface)?;
    // Kill switches only refuse packets that would leave the device, so TTL 0
    // can't tell; TTL 1 keeps the probe on the LAN.
    socket.set_multicast_ttl_v4(1)?;
    socket.set_multicast_loop_v4(false)?;
    socket.set_nonblocking(true)?;
    let socket = UdpSocket::from_std(socket.into())?;
    match socket.send_to(&EMPTY_DNS_HEADER, MDNS_V4).await {
        Ok(_) => Ok(false),
        // Local Network privacy and reject routes answer EHOSTUNREACH, kill
        // switches ENETUNREACH, firewalls EPERM. Other errors, like ENOBUFS, say
        // nothing about the LAN.
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::HostUnreachable
                    | io::ErrorKind::NetworkUnreachable
                    | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(true)
        }
        Err(err) => Err(err),
    }
}

fn log_change(blocked: bool) {
    if blocked {
        log::warn!("mDNS can't reach the LAN: multicast is refused");
    } else {
        log::info!("mDNS reaches the LAN again");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_not_set_up() -> io::Result<bool> {
        Err(io::Error::from(io::ErrorKind::OutOfMemory))
    }

    #[test]
    fn a_device_with_no_lan_is_offline_not_blocked() {
        assert_eq!(every_lan_refused(vec![]), Some(false));
    }

    #[test]
    fn one_lan_that_takes_multicast_is_enough() {
        assert_eq!(every_lan_refused(vec![Ok(true), Ok(false)]), Some(false));
    }

    #[test]
    fn every_lan_refusing_multicast_is_a_refusal() {
        assert_eq!(every_lan_refused(vec![Ok(true), Ok(true)]), Some(true));
    }

    #[test]
    fn a_probe_that_could_not_be_set_up_says_nothing() {
        assert_eq!(every_lan_refused(vec![Ok(true), probe_not_set_up()]), None);
    }
}
