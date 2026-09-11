//! Browsing for local hubs on the LAN over mDNS (swarm-discovery): every
//! sighting is TCP-probed, and a hub is reported found once it answers and lost
//! once it doesn't, whether because a probe failed or because it aged out of
//! the swarm. One browser lives as long as the service, kept joined to the
//! current interfaces across network changes so its view of the swarm (and so
//! its expiry) carries across them.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::future::ready;
use futures::stream::{BoxStream, Stream, StreamExt};
use swarm_discovery::DropGuard;
use tokio::sync::broadcast;
use tokio_util::task::AbortOnDropHandle;

use crate::{base_discoverer, label_to_mailbox_id, multicast_interfaces_v4, SERVICE_NAME};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// How many sightings are probed at once; the rest wait their turn.
const MAX_PROBES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalHubEvent {
    /// A hub became reachable on the LAN, or answered again at a possibly new
    /// url after a network change. `id` is the announcer's instance name (the
    /// hub's MailboxId); `url` is an `http://host:port` that accepted a TCP
    /// connection.
    Found { id: String, url: String },
    /// A hub aged out of the swarm, or stopped answering.
    Lost { id: String },
}

/// A hub as the browser saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sighting {
    /// The announcer's instance name (the hub's MailboxId).
    id: String,
    /// The advertised addresses; none once the hub aged out of the swarm.
    addrs: Vec<(IpAddr, u16)>,
    /// How many network changes came before it. A hub found before a change
    /// thus reads as sighted anew after it, and is probed and reported found
    /// again, so the mailbox layer refreshes our own address with it.
    network_changes: u64,
}

/// A swarm-discovery browser, with the interfaces it has joined.
type Browser = (DropGuard, BTreeSet<Ipv4Addr>);

pub struct LocalHubDiscoveryService {
    events: BoxStream<'static, LocalHubEvent>,
    _browser: AbortOnDropHandle<()>,
}

impl LocalHubDiscoveryService {
    /// Browse for local hubs until the returned service is dropped.
    pub fn spawn() -> Self {
        log::info!("Started local hub discovery (swarm-discovery, {SERVICE_NAME})");
        let (sightings_tx, sightings) = unbounded();
        let browser = AbortOnDropHandle::new(tokio::spawn(browse(sightings_tx)));
        Self::new(sightings, browser)
    }

    /// Probe every sighting concurrently; a hub is found once it answers and
    /// lost once it doesn't. `browser` is stopped with the service.
    fn new(
        sightings: impl Stream<Item = Sighting> + Send + 'static,
        browser: AbortOnDropHandle<()>,
    ) -> Self {
        let mut found = FoundHubs::default();
        let events = sightings
            .map(|sighting| async move {
                let url = probe_reachable(&sighting.addrs).await;
                (sighting, url)
            })
            .buffer_unordered(MAX_PROBES)
            .filter_map(move |(sighting, url)| ready(found.update(sighting, url)))
            .boxed();
        Self {
            events,
            _browser: browser,
        }
    }

    /// The next discovery event, or `None` once discovery has stopped.
    pub async fn recv(&mut self) -> Option<LocalHubEvent> {
        self.events.next().await
    }
}

/// One browser for the whole run, kept on the current interfaces across
/// network changes (or spawned on one, if there was no routable IPv4 to spawn
/// it on before). Sightings are stamped with the network changes so far.
async fn browse(sightings: UnboundedSender<Sighting>) {
    let network_changes = Arc::new(AtomicU64::new(0));
    let mut network = network_watch::network_change();
    let mut browser: Option<Browser> = None;
    loop {
        browser = match browser {
            Some(browser) => Some(update_interfaces(browser)),
            None => spawn_browser(&sightings, &network_changes)
                .inspect_err(|err| {
                    log::warn!(
                        "Failed to bind local hub discovery (retrying on next network change): {err}"
                    )
                })
                .ok(),
        };
        match network.recv().await {
            Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                network_changes.fetch_add(1, Ordering::Relaxed);
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::warn!(
                    "Network change signal closed; local hub discovery stays on the current interfaces"
                );
                return;
            }
        }
    }
}

/// A swarm-discovery browser on the current interfaces, forwarding every
/// sighting to `sightings`; fails if binding the multicast socket does.
fn spawn_browser(
    sightings: &UnboundedSender<Sighting>,
    network_changes: &Arc<AtomicU64>,
) -> anyhow::Result<Browser> {
    let interfaces: BTreeSet<Ipv4Addr> = multicast_interfaces_v4().into_iter().collect();
    let sightings = sightings.clone();
    let network_changes = network_changes.clone();
    let discoverer = base_discoverer(&browse_id(), interfaces.iter().copied().collect())
        // Runs on swarm-discovery's thread and must not block, so it only
        // forwards the sighting.
        .with_callback(move |id, peer| {
            let Ok(id) = label_to_mailbox_id(id) else {
                return;
            };
            let _ = sightings.unbounded_send(Sighting {
                id,
                addrs: peer.addrs().to_vec(),
                network_changes: network_changes.load(Ordering::Relaxed),
            });
        });
    let guard = discoverer.spawn(&tokio::runtime::Handle::current())?;
    Ok((guard, interfaces))
}

/// Join the interfaces that appeared since the browser last joined and leave
/// the ones that went, so the one browser (and its view of the swarm) carries
/// across network changes.
fn update_interfaces((guard, joined): Browser) -> Browser {
    let current: BTreeSet<Ipv4Addr> = multicast_interfaces_v4().into_iter().collect();
    for gone in joined.difference(&current) {
        guard.remove_interface_v4(*gone);
    }
    for new in current.difference(&joined) {
        guard.add_interface_v4(*new);
    }
    (guard, current)
}

/// A per-process browse id (a valid DNS label). swarm-discovery needs one even to
/// browse; ours only has to not collide with a hub's MailboxId.
fn browse_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("browse-{}-{}", std::process::id(), n)
}

/// The hubs reported found so far, each as the sighting it was found from.
#[derive(Default)]
struct FoundHubs(BTreeMap<String, Sighting>);

impl FoundHubs {
    /// The event a probe of `sighting` calls for, `url` being where it
    /// answered: `Found` for a hub not found yet or since sighted differently,
    /// `Lost` for one that aged out of the swarm or stopped answering at the
    /// very addresses it was found at. Probes finish in any order, so a failure
    /// at other (stale) addresses says nothing about where it was found since.
    fn update(&mut self, sighting: Sighting, url: Option<String>) -> Option<LocalHubEvent> {
        let expired = sighting.addrs.is_empty();
        match (url, self.0.get(&sighting.id)) {
            (Some(_), Some(known)) if *known == sighting => None,
            (Some(url), _) => {
                let id = sighting.id.clone();
                self.0.insert(id.clone(), sighting);
                Some(LocalHubEvent::Found { id, url })
            }
            (None, Some(known)) if expired || known.addrs == sighting.addrs => {
                self.0.remove(&sighting.id);
                Some(LocalHubEvent::Lost { id: sighting.id })
            }
            (None, _) => None,
        }
    }
}

/// TCP-probe a hub's advertised addresses
async fn probe_reachable(addrs: &[(IpAddr, u16)]) -> Option<String> {
    let (loopback, routable): (Vec<_>, Vec<_>) =
        addrs.iter().copied().partition(|(ip, _)| ip.is_loopback());
    for group in [routable, loopback] {
        if let Some(addr) = race_connect(&group).await {
            return Some(format!("http://{addr}"));
        }
    }
    None
}

/// The first of `addrs` to accept a connection, all raced together
async fn race_connect(addrs: &[(IpAddr, u16)]) -> Option<SocketAddr> {
    if addrs.is_empty() {
        return None;
    }
    let probes = addrs.iter().map(|&(ip, port)| {
        let addr = SocketAddr::from((ip, port));
        Box::pin(async move { probe_tcp(addr).await.then_some(addr).ok_or(()) })
    });
    futures::future::select_ok(probes)
        .await
        .ok()
        .map(|(addr, _)| addr)
}

async fn probe_tcp(addr: SocketAddr) -> bool {
    match tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => true,
        Ok(Err(err)) => {
            log::trace!("Probe to {addr} refused / errored: {err}");
            false
        }
        Err(_elapsed) => {
            log::trace!("Probe to {addr} timed out after {PROBE_TIMEOUT:?}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use tokio::net::TcpListener;

    use super::*;

    /// A blackhole (TEST-NET-1): connecting hangs until `PROBE_TIMEOUT`.
    const DEAD_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

    /// The pipe fed sightings and network changes by hand, with no browser.
    struct Harness {
        sightings: UnboundedSender<Sighting>,
        network_changes: u64,
        service: LocalHubDiscoveryService,
    }

    impl Harness {
        fn new() -> Self {
            let (sightings, sightings_rx) = unbounded();
            Self {
                sightings,
                network_changes: 0,
                service: LocalHubDiscoveryService::new(
                    sightings_rx,
                    AbortOnDropHandle::new(tokio::spawn(async {})),
                ),
            }
        }

        fn seen(&self, id: &str, addr: SocketAddr) {
            self.sighted(id, vec![(addr.ip(), addr.port())]);
        }

        fn expired(&self, id: &str) {
            self.sighted(id, vec![]);
        }

        fn sighted(&self, id: &str, addrs: Vec<(IpAddr, u16)>) {
            let _ = self.sightings.unbounded_send(Sighting {
                id: id.to_string(),
                addrs,
                network_changes: self.network_changes,
            });
        }

        fn network_changed(&mut self) {
            self.network_changes += 1;
        }

        async fn next(&mut self) -> LocalHubEvent {
            tokio::time::timeout(Duration::from_secs(10), self.service.recv())
                .await
                .expect("an event")
                .expect("discovery running")
        }

        /// The next two events, sorted, since probes finish in any order.
        async fn next_two(&mut self) -> Vec<LocalHubEvent> {
            sorted(vec![self.next().await, self.next().await])
        }

        /// No event for long enough that every in-flight probe has finished.
        async fn quiet(&mut self) {
            let event = tokio::time::timeout(PROBE_TIMEOUT * 2, self.service.recv()).await;
            assert!(event.is_err(), "unexpected event: {event:?}");
        }
    }

    async fn listen() -> TcpListener {
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap()
    }

    fn found(id: &str, addr: SocketAddr) -> LocalHubEvent {
        LocalHubEvent::Found {
            id: id.to_string(),
            url: format!("http://{addr}"),
        }
    }

    fn lost(id: &str) -> LocalHubEvent {
        LocalHubEvent::Lost { id: id.to_string() }
    }

    fn sorted(mut events: Vec<LocalHubEvent>) -> Vec<LocalHubEvent> {
        events.sort_by_key(|event| format!("{event:?}"));
        events
    }

    #[tokio::test]
    async fn a_hub_is_found_once_however_often_it_is_sighted() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, found("hub", hub_addr));

        let other = listen().await;
        let other_addr = other.local_addr().unwrap();
        h.seen("hub", hub_addr);
        h.seen("hub", hub_addr);
        h.seen("other", other_addr);
        assert_eq!(h.next().await, found("other", other_addr));
    }

    #[tokio::test]
    async fn an_unreachable_hub_does_not_delay_a_reachable_one() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("dead", DEAD_ADDR);
        h.seen("hub", hub_addr);

        let started = Instant::now();
        assert_eq!(h.next().await, found("hub", hub_addr));
        assert!(started.elapsed() < PROBE_TIMEOUT);
    }

    #[tokio::test]
    async fn a_hub_sighted_at_new_addresses_is_found_again_there() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, found("hub", hub_addr));

        let moved = listen().await;
        let moved_addr = moved.local_addr().unwrap();
        h.seen("hub", moved_addr);
        assert_eq!(h.next().await, found("hub", moved_addr));
    }

    #[tokio::test]
    async fn a_stale_probe_failing_late_does_not_lose_a_hub_found_again_elsewhere() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        let old_addrs = vec![
            (DEAD_ADDR.ip(), DEAD_ADDR.port()),
            (hub_addr.ip(), hub_addr.port()),
        ];
        h.sighted("hub", old_addrs.clone());
        assert_eq!(h.next().await, found("hub", hub_addr));

        let moved = listen().await;
        let moved_addr = moved.local_addr().unwrap();
        drop(hub);
        h.sighted("hub", old_addrs);
        h.seen("hub", moved_addr);
        assert_eq!(h.next().await, found("hub", moved_addr));
        h.quiet().await;
    }

    #[tokio::test]
    async fn an_expired_hub_is_lost_only_if_it_was_found() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, found("hub", hub_addr));

        h.seen("dead", DEAD_ADDR);
        h.expired("dead");
        h.expired("hub");
        assert_eq!(h.next().await, lost("hub"));
    }

    #[tokio::test]
    async fn after_a_network_change_a_survivor_is_found_again_and_a_dead_hub_lost() {
        let mut h = Harness::new();
        let survivor = listen().await;
        let survivor_addr = survivor.local_addr().unwrap();
        let dead = listen().await;
        let dead_addr = dead.local_addr().unwrap();
        h.seen("survivor", survivor_addr);
        h.seen("dead", dead_addr);
        assert_eq!(
            h.next_two().await,
            sorted(vec![
                found("survivor", survivor_addr),
                found("dead", dead_addr)
            ])
        );

        drop(dead);
        h.network_changed();
        h.seen("survivor", survivor_addr);
        h.seen("dead", dead_addr);
        assert_eq!(
            h.next_two().await,
            sorted(vec![found("survivor", survivor_addr), lost("dead")])
        );
    }
}
