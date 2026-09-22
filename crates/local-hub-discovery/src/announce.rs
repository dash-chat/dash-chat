//! Advertising a local hub on the LAN over mDNS (swarm-discovery), so browsers
//! on the same network can discover it.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use swarm_discovery::DropGuard;
use tokio::sync::{broadcast, Mutex};
use tokio_util::task::AbortOnDropHandle;

use crate::{
    base_discoverer, mailbox_id_to_label, multicast_interfaces_v4, service_name, GOODBYE_ATTRIBUTE,
};

/// One announce round, charged to every shutdown.
const GOODBYE_LINGER: Duration = Duration::from_millis(1000);

pub struct LocalHubAnnouncementService {
    announcement: Arc<Mutex<Option<DropGuard>>>,
    _reannouncer: AbortOnDropHandle<()>,
}

impl LocalHubAnnouncementService {
    /// Announce a local hub on the LAN over mDNS (swarm-discovery), so browsers on
    /// the same network discover it. `instance_id` is the swarm id peers see;
    /// `port` is where the hub listens on every interface. Every routable local
    /// IPv4 is advertised (loopback only if there is none), re-enumerated on each
    /// network change. Must be called within a Tokio runtime.
    pub fn spawn(instance_id: &str, port: u16) -> anyhow::Result<Self> {
        let handle = tokio::runtime::Handle::current();
        // Eager first announce so a bad runtime or bind fails fast.
        let initial = announce(instance_id, port, &handle)?;
        let announcement = Arc::new(Mutex::new(Some(initial)));
        let instance_id = instance_id.to_string();
        let live = announcement.clone();
        // swarm-discovery pins its multicast socket and advertised addresses at
        // spawn, so re-announce on every network change — as the browser re-binds.
        let reannouncer = AbortOnDropHandle::new(tokio::spawn(async move {
            let mut network = network_watch::network_change();
            loop {
                match network.recv().await {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {
                        log::warn!(
                            "Network change signal closed; local hub {instance_id} keeps its current announcement"
                        );
                        return;
                    }
                }
                // `announce` multicasts as soon as it returns, and one
                // without the goodbye must not go out mid-shutdown.
                let mut live = live.lock().await;
                if live.is_none() {
                    return;
                }
                match announce(&instance_id, port, &handle) {
                    Ok(next) => *live = Some(next),
                    Err(err) => log::warn!(
                        "Failed to re-announce local hub {instance_id} (keeping the previous announcement, retrying on next network change): {err}"
                    ),
                }
            }
        }));
        Ok(Self {
            announcement,
            _reannouncer: reannouncer,
        })
    }

    /// Announce that this hub is going away, then stop announcing.
    pub async fn shutdown(self) {
        let mut announcement = self.announcement.lock().await;
        let Some(live) = announcement.as_ref() else {
            return;
        };
        match live.set_txt_attribute(GOODBYE_ATTRIBUTE.to_string(), None) {
            // swarm-discovery sends on its own schedule: this waits for a
            // round, not for an acknowledgement.
            Ok(()) if announces_off_host() => tokio::time::sleep(GOODBYE_LINGER).await,
            Ok(()) => {}
            Err(err) => log::warn!("Failed to announce local hub goodbye: {err}"),
        }
        announcement.take();
    }
}

/// Spawn a swarm-discovery announcer for a hub listening on `port`.
fn announce(
    instance_id: &str,
    port: u16,
    handle: &tokio::runtime::Handle,
) -> anyhow::Result<DropGuard> {
    let interfaces = multicast_interfaces_v4();
    let ips = announce_ips(&interfaces);
    log::info!(
        "Announcing local hub {instance_id} on the LAN via swarm-discovery ({}) at {ips:?}:{port}",
        service_name()
    );
    let guard = base_discoverer(&mailbox_id_to_label(instance_id)?, interfaces)
        .with_addrs(port, ips)
        .spawn(handle)?;
    Ok(guard)
}

fn announces_off_host() -> bool {
    multicast_interfaces_v4().iter().any(|ip| !ip.is_loopback())
}

/// The IPv4 addresses to advertise: the interfaces multicast goes out on
/// (mDNS is IPv4-only here, see [`crate::base_discoverer`]), minus loopback
/// whenever there is any other, since a browser elsewhere on the LAN would
/// take a 127.0.0.1 it hears to mean its own host.
fn announce_ips(interfaces: &[Ipv4Addr]) -> Vec<IpAddr> {
    let (loopback, routable): (Vec<Ipv4Addr>, Vec<Ipv4Addr>) =
        interfaces.iter().copied().partition(|v4| v4.is_loopback());
    let ips = if routable.is_empty() {
        loopback
    } else {
        routable
    };
    ips.into_iter().map(IpAddr::V4).collect()
}
