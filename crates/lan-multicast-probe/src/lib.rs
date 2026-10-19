//! Whether this device's mDNS multicast can leave it at all, which mDNS
//! libraries only log about. macOS refuses it with EHOSTUNREACH to an app
//! without Local Network access (TN3179), and so does a VPN that leaves a
//! reject route on 224.0.0.0/4: nobody on the LAN can discover the device.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::OnceLock;
use std::time::Duration;

use futures::Stream;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

/// How often to probe: on macOS, Local Network access can be granted or
/// revoked at any time.
const PROBE_INTERVAL: Duration = Duration::from_secs(3);

/// Rounds in a row every LAN must refuse before the LAN counts as blocked, so
/// a refusal during a network change raises no alarm.
const BLOCKED_ROUNDS: u32 = 2;

const MDNS_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);

/// A DNS header asking nothing, which mDNS responders ignore.
const EMPTY_QUERY: [u8; 12] = [0; 12];

/// Whether every LAN this device is on refuses its mDNS multicast: the current
/// verdict, then every change. One probe serves every watcher, and it only
/// probes while someone watches. Must be called within a Tokio runtime.
pub fn lan_multicast_blocked() -> impl Stream<Item = bool> {
    WatchStream::new(shared_verdict().subscribe())
}

fn shared_verdict() -> &'static watch::Sender<bool> {
    static VERDICT: OnceLock<watch::Sender<bool>> = OnceLock::new();
    VERDICT.get_or_init(|| {
        let verdict = watch::Sender::new(false);
        tokio::spawn(probe_while_watched(verdict.clone()));
        verdict
    })
}

async fn probe_while_watched(verdict: watch::Sender<bool>) {
    let mut refused_rounds = 0;
    loop {
        if verdict.receiver_count() > 0 {
            match probe_every_lan().await {
                Some(true) => refused_rounds += 1,
                Some(false) => refused_rounds = 0,
                None => {}
            }
            let blocked = refused_rounds >= BLOCKED_ROUNDS;
            if *verdict.borrow() != blocked {
                log_change(blocked);
                verdict.send_replace(blocked);
            }
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
}

/// Whether every LAN refused this round's probe, or `None` if that can't be
/// told.
async fn probe_every_lan() -> Option<bool> {
    let lans: Vec<Ipv4Addr> = local_hub_discovery::multicast_interfaces_v4()
        .into_iter()
        .filter(|ip| !ip.is_loopback())
        .collect();
    // Socket calls block.
    tokio::task::spawn_blocking(move || {
        every_lan_refused(lans.into_iter().map(refuses_multicast).collect())
    })
    .await
    .ok()
    .flatten()
}

/// One LAN taking multicast is enough, a device with no LAN is offline rather
/// than blocked, and a probe that couldn't be set up says nothing.
fn every_lan_refused(probes: Vec<std::io::Result<bool>>) -> Option<bool> {
    if probes.is_empty() || probes.iter().any(|probe| matches!(probe, Ok(false))) {
        Some(false)
    } else if probes.iter().any(|probe| probe.is_err()) {
        None
    } else {
        Some(true)
    }
}

/// Whether `interface` refuses our multicast. Failing to set up the socket
/// (out of descriptors, say) is an error, not a refusal.
fn refuses_multicast(interface: Ipv4Addr) -> std::io::Result<bool> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_if_v4(&interface)?;
    Ok(socket.send_to(&EMPTY_QUERY, &MDNS_V4.into()).is_err())
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
    use std::io;

    use super::*;

    fn unset_up() -> io::Result<bool> {
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
        assert_eq!(every_lan_refused(vec![Ok(true), unset_up()]), None);
    }
}
