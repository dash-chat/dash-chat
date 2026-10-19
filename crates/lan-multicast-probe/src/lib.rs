//! Whether this device's mDNS multicast can leave it at all, which mDNS
//! libraries only log about. macOS refuses it with EHOSTUNREACH to an app
//! without Local Network access (TN3179), and so does a VPN that leaves a
//! reject route on 224.0.0.0/4: nobody on the LAN can discover the device.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;

/// How often to probe: on macOS, Local Network access can be granted or
/// revoked at any time.
const PROBE_INTERVAL: Duration = Duration::from_secs(3);

const MDNS_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);

/// A DNS header asking nothing, which mDNS responders ignore.
const EMPTY_QUERY: [u8; 12] = [0; 12];

/// Probes the LAN in the background until its last clone is dropped.
#[derive(Clone)]
pub struct LanMulticastProbe {
    blocked: watch::Receiver<bool>,
    _prober: Arc<AbortOnDropHandle<()>>,
}

impl LanMulticastProbe {
    /// Must be called within a Tokio runtime.
    pub fn spawn() -> Self {
        let (blocked_tx, blocked) = watch::channel(false);
        let prober = AbortOnDropHandle::new(tokio::spawn(probe(blocked_tx)));
        Self {
            blocked,
            _prober: Arc::new(prober),
        }
    }

    /// Whether every LAN this device is on refuses its mDNS multicast.
    pub fn blocked(&self) -> watch::Receiver<bool> {
        self.blocked.clone()
    }
}

async fn probe(blocked: watch::Sender<bool>) {
    loop {
        let now_blocked = lan_multicast_blocked();
        if blocked.send_if_modified(|was| std::mem::replace(was, now_blocked) != now_blocked) {
            if now_blocked {
                log::warn!("mDNS can't reach the LAN: multicast is refused");
            } else {
                log::info!("mDNS reaches the LAN again");
            }
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
}

/// A device with no LAN is not blocked, only offline.
fn lan_multicast_blocked() -> bool {
    let lans = lan_interfaces_v4();
    if lans.is_empty() {
        return false;
    }
    lans.into_iter().all(refuses_multicast)
}

/// The IPv4 interfaces mDNS goes out on to reach other devices.
fn lan_interfaces_v4() -> Vec<Ipv4Addr> {
    let interfaces = if_addrs::get_if_addrs().unwrap_or_default();
    interfaces
        .iter()
        .filter(|interface| !interface.is_p2p())
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => Some(v4),
            _ => None,
        })
        .collect()
}

fn refuses_multicast(interface: Ipv4Addr) -> bool {
    send_probe(interface).is_err()
}

fn send_probe(interface: Ipv4Addr) -> std::io::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_if_v4(&interface)?;
    socket.send_to(&EMPTY_QUERY, &MDNS_V4.into())?;
    Ok(())
}
