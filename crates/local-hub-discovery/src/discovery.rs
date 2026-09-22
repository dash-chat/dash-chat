//! Browsing for local hubs on the LAN over mDNS (swarm-discovery): every
//! sighting is TCP-probed, and the hubs that answered are published as a set
//! for consumers to reconcile against. One browser lives as long as the
//! service, kept joined to the current interfaces across network changes so its
//! view of the swarm (and so its expiry) carries across them.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use swarm_discovery::DropGuard;
use tokio::sync::{broadcast, watch};
use tokio_util::task::AbortOnDropHandle;

use crate::{
    base_discoverer, label_to_mailbox_id, local_subnets_v4, multicast_interfaces_v4,
    GOODBYE_ATTRIBUTE,
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// Longer than [`PROBE_TIMEOUT`], so a hub with an address that hangs is not
/// re-probed before the sweep that is still waiting on it gives up.
const REPROBE_INTERVAL: Duration = Duration::from_secs(PROBE_TIMEOUT.as_secs() + 1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredHub {
    pub mailbox_id: String,
    pub answered_at: Vec<SocketAddr>,
}

pub struct LocalHubDiscoveryService {
    browser: Arc<DiscoveryBrowser>,
    _rejoining: AbortOnDropHandle<()>,
}

impl LocalHubDiscoveryService {
    /// Browse for local hubs until the returned service is dropped.
    pub fn spawn(service_name: &str) -> Self {
        log::info!("Started local hub discovery (swarm-discovery, {service_name})");
        let browser = DiscoveryBrowser::new(service_name);
        Self {
            _rejoining: AbortOnDropHandle::new(tokio::spawn(Self::rejoin_on_network_changes(
                browser.clone(),
            ))),
            browser,
        }
    }

    pub fn hubs(&self) -> watch::Receiver<BTreeMap<String, DiscoveredHub>> {
        self.browser.published.subscribe()
    }

    async fn rejoin_on_network_changes(browser: Arc<DiscoveryBrowser>) {
        let mut network = network_watch::network_change();
        loop {
            browser.rejoin();
            match network.recv().await {
                Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {
                    log::warn!(
                        "Network change signal closed; local hub discovery stays on the current interfaces"
                    );
                    return;
                }
            }
        }
    }
}

#[derive(Default)]
struct Hub {
    answered_at: Vec<SocketAddr>,
    probed_addrs: BTreeSet<SocketAddr>,
    awaiting_probe: Option<u64>,
    probed_at: Option<Instant>,
}

impl Hub {
    fn needs_probe(&self, advertised: &BTreeSet<SocketAddr>) -> bool {
        if self.awaiting_probe.is_some() {
            return false;
        }
        match self.probed_at {
            Some(at) if self.probed_addrs == *advertised => at.elapsed() >= REPROBE_INTERVAL,
            _ => true,
        }
    }
}

struct Browsing {
    guard: DropGuard,
    joined: BTreeSet<Ipv4Addr>,
}

struct DiscoveryBrowser {
    /// The swarm both halves meet on; a test gives itself one of its own so it
    /// cannot be discovered by real clients on the LAN it runs on.
    service_name: String,
    hubs: Mutex<BTreeMap<String, Hub>>,
    published: watch::Sender<BTreeMap<String, DiscoveredHub>>,
    /// One for the whole run, so its view of the swarm (and so its expiry)
    /// carries across network changes.
    browsing: Mutex<Option<Browsing>>,
}

impl DiscoveryBrowser {
    fn new(service_name: &str) -> Arc<Self> {
        Arc::new(Self {
            service_name: service_name.to_string(),
            hubs: Mutex::default(),
            published: watch::channel(BTreeMap::new()).0,
            browsing: Mutex::default(),
        })
    }

    fn rejoin(self: &Arc<Self>) {
        self.forget_probes_that_cannot_answer();
        let current: BTreeSet<Ipv4Addr> = multicast_interfaces_v4().into_iter().collect();
        let mut browsing = self
            .browsing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(Browsing { guard, joined }) = browsing.as_mut() else {
            log::debug!("Browsing for local hubs on {current:?}");
            *browsing = self
                .start_browsing(current)
                .inspect_err(|err| {
                    log::warn!(
                        "Failed to bind local hub discovery (retrying on next network change): {err}"
                    )
                })
                .ok();
            return;
        };
        if *joined == current {
            return;
        }
        log::debug!("Browsing for local hubs on {current:?} (was {joined:?})");
        for gone in joined.difference(&current) {
            guard.remove_interface_v4(*gone);
        }
        for new in current.difference(joined) {
            guard.add_interface_v4(*new);
        }
        *joined = current;
    }

    /// A probe dialling addresses none of our networks can reach now cannot
    /// answer, and while it waits no sighting of that hub is probed at all.
    /// One still dialling an address we can reach is left to finish: sightings
    /// routinely arrive before we notice the network changed, and that probe
    /// is the fastest answer we are going to get.
    fn forget_probes_that_cannot_answer(&self) {
        let subnets = local_subnets_v4();
        for hub in self
            .hubs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values_mut()
        {
            if !hub
                .probed_addrs
                .iter()
                .any(|addr| reachable_from_here(*addr, &subnets))
            {
                hub.awaiting_probe = None;
            }
            hub.probed_at = None;
        }
    }

    fn start_browsing(
        self: &Arc<Self>,
        interfaces: BTreeSet<Ipv4Addr>,
    ) -> anyhow::Result<Browsing> {
        let browser = Arc::downgrade(self);
        let discoverer = base_discoverer(
            &self.service_name,
            &browse_id(),
            interfaces.iter().copied().collect(),
        )
        // Runs on a swarm-discovery actor, which is spawned on the handle
        // passed to `Discoverer::spawn` below — so this is on our runtime
        // and may spawn, but must not block: it only takes the sighting in.
        .with_callback(move |id, peer| {
            let Ok(id) = label_to_mailbox_id(id) else {
                return;
            };
            let Some(browser) = browser.upgrade() else {
                return;
            };
            browser.sighted(
                id,
                peer.addrs().iter().map(|&a| SocketAddr::from(a)).collect(),
                peer.txt_attribute(GOODBYE_ATTRIBUTE).is_some(),
            );
        });
        let guard = discoverer.spawn(&tokio::runtime::Handle::current())?;
        Ok(Browsing {
            guard,
            joined: interfaces,
        })
    }

    fn sighted(self: &Arc<Self>, id: String, advertised: BTreeSet<SocketAddr>, goodbye: bool) {
        let mut hubs = self
            .hubs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // An unanswered probe is deliberately not a third way to be gone: it
        // looks exactly like our own network being down.
        if goodbye || advertised.is_empty() {
            // Dropping it drops what the probe it was waiting on would have
            // said, so one still in flight cannot bring it back.
            if hubs.remove(&id).is_some() {
                if goodbye {
                    log::debug!("Local hub is gone, it said goodbye: mailbox={id}");
                } else {
                    log::debug!("Local hub is gone, its announcements lapsed: mailbox={id}");
                }
                self.publish(&hubs);
            }
            return;
        }
        let hub = hubs.entry(id.clone()).or_default();
        if !hub.needs_probe(&advertised) {
            return;
        }
        log::debug!("Local hub sighted at {advertised:?}, probing: mailbox={id}");
        // One probe in flight per hub, however often it is sighted, and a LAN
        // carries a handful of hubs: the fan-out needs no bound of its own.
        let probe_id = next_probe_id();
        hub.awaiting_probe = Some(probe_id);
        hub.probed_at = Some(Instant::now());
        hub.probed_addrs = advertised.clone();
        tokio::spawn(self.clone().probe_hub(id, probe_id, advertised));
    }

    async fn probe_hub(
        self: Arc<Self>,
        id: String,
        probe_id: u64,
        advertised: BTreeSet<SocketAddr>,
    ) {
        let mut probes: FuturesUnordered<_> = advertised
            .into_iter()
            .map(|addr| async move { answers(addr).await.then_some(addr) })
            .collect();
        let mut answered = Vec::new();
        while let Some(answer) = probes.next().await {
            let Some(addr) = answer else { continue };
            answered.push(addr);
            if answered.len() == 1 {
                self.answered(&id, probe_id, answered.clone(), false);
            }
        }
        let subnets = local_subnets_v4();
        answered.sort_by_key(|addr| (hops_away(*addr, &subnets), *addr));
        self.answered(&id, probe_id, answered, true);
    }

    /// A hub only takes the results of the probe it is waiting on, so one
    /// outlived by a newer sighting cannot speak for it.
    fn answered(&self, id: &str, probe_id: u64, answered_at: Vec<SocketAddr>, swept: bool) {
        let mut hubs = self
            .hubs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(hub) = hubs.get_mut(id) else {
            return;
        };
        if hub.awaiting_probe != Some(probe_id) {
            return;
        }
        if swept {
            hub.awaiting_probe = None;
        }
        // Answering nowhere says nothing about whether the hub is still there.
        if answered_at.is_empty() {
            return;
        }
        // A first answer is only enough to publish a hub we had nothing for;
        // replacing what we know is for the finished sweep.
        if !swept && !hub.answered_at.is_empty() {
            return;
        }
        if hub.answered_at.is_empty() {
            log::debug!("Local hub answered at {}: mailbox={id}", answered_at[0]);
        }
        hub.answered_at = answered_at;
        self.publish(&hubs);
    }

    fn publish(&self, hubs: &BTreeMap<String, Hub>) {
        let answering: BTreeMap<String, DiscoveredHub> = hubs
            .iter()
            .filter(|(_, hub)| !hub.answered_at.is_empty())
            .map(|(id, hub)| {
                (
                    id.clone(),
                    DiscoveredHub {
                        mailbox_id: id.clone(),
                        answered_at: hub.answered_at.clone(),
                    },
                )
            })
            .collect();
        self.published.send_if_modified(|published| {
            let changed = answering != *published;
            *published = answering;
            changed
        });
    }
}

/// How far an address is from us: on a subnet we are on, elsewhere, or
/// loopback — which a hub on another host would have meant its own by.
/// Whether a probe to this address could still be answered from where we are
/// now: our own host, or a subnet we hold an address on.
fn reachable_from_here(addr: SocketAddr, subnets: &[(Ipv4Addr, u8)]) -> bool {
    match addr.ip() {
        IpAddr::V4(v4) => v4.is_loopback() || on_a_local_subnet(v4, subnets),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

fn hops_away(addr: SocketAddr, subnets: &[(Ipv4Addr, u8)]) -> u8 {
    match addr.ip() {
        ip if ip.is_loopback() => 2,
        IpAddr::V4(v4) if on_a_local_subnet(v4, subnets) => 0,
        _ => 1,
    }
}

fn on_a_local_subnet(ip: Ipv4Addr, subnets: &[(Ipv4Addr, u8)]) -> bool {
    subnets.iter().any(|&(local, prefix)| {
        (1..=32).contains(&prefix)
            && u32::from(ip) >> (32 - prefix) == u32::from(local) >> (32 - prefix)
    })
}

async fn answers(addr: SocketAddr) -> bool {
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

/// Unique for the process, so a hub that left and came back cannot take the
/// results of a probe started before it went.
fn next_probe_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// swarm-discovery needs one even to browse; ours only has to be a valid DNS
/// label that cannot collide with a hub's MailboxId.
fn browse_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("browse-{}-{}", std::process::id(), n)
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::time::Instant;

    use tokio::net::TcpListener;

    use super::*;

    /// A blackhole (TEST-NET-1): connecting hangs until `PROBE_TIMEOUT`.
    const DEAD_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

    /// The state fed sightings by hand, with no browser.
    struct Harness {
        browser: Arc<DiscoveryBrowser>,
        hubs: watch::Receiver<BTreeMap<String, DiscoveredHub>>,
    }

    impl Harness {
        /// The browser fed sightings by hand, with no discoverer behind it.
        fn new() -> Self {
            let browser = DiscoveryBrowser::new("dashchat-unit-test");
            let hubs = browser.published.subscribe();
            Self { browser, hubs }
        }

        fn seen(&mut self, id: &str, addr: SocketAddr) {
            self.sighted(id, vec![addr]);
        }

        fn expired(&mut self, id: &str) {
            self.sighted(id, vec![]);
        }

        fn said_goodbye(&mut self, id: &str, addr: SocketAddr) {
            self.send(id, vec![addr], true);
        }

        fn sighted(&mut self, id: &str, addrs: Vec<SocketAddr>) {
            self.send(id, addrs, false);
        }

        fn send(&mut self, id: &str, addrs: Vec<SocketAddr>, goodbye: bool) {
            self.browser
                .sighted(id.to_string(), addrs.into_iter().collect(), goodbye);
        }

        /// The set once it next changes.
        async fn next(&mut self) -> BTreeMap<String, DiscoveredHub> {
            tokio::time::timeout(Duration::from_secs(10), self.hubs.changed())
                .await
                .expect("the set to change")
                .expect("discovery running");
            self.hubs.borrow_and_update().clone()
        }

        /// No change for long enough that every in-flight probe has finished.
        async fn unchanged(&mut self) {
            let changed = tokio::time::timeout(PROBE_TIMEOUT * 2, self.hubs.changed()).await;
            assert!(
                changed.is_err(),
                "unexpected change: {:?}",
                self.hubs.borrow()
            );
        }
    }

    async fn listen() -> TcpListener {
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap()
    }

    fn hubs(entries: &[(&str, SocketAddr)]) -> BTreeMap<String, DiscoveredHub> {
        entries
            .iter()
            .map(|(id, addr)| {
                (
                    id.to_string(),
                    DiscoveredHub {
                        mailbox_id: id.to_string(),
                        answered_at: vec![*addr],
                    },
                )
            })
            .collect()
    }

    fn at(listener: &TcpListener) -> SocketAddr {
        listener.local_addr().unwrap()
    }

    #[tokio::test]
    async fn a_hub_that_answers_joins_the_set_once_however_often_it_is_sighted() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));

        h.seen("hub", hub_addr);
        h.seen("hub", hub_addr);
        h.unchanged().await;
    }

    #[tokio::test]
    async fn a_hub_joins_on_its_first_answer_and_the_rest_fill_in_after() {
        let mut h = Harness::new();
        let one = listen().await;
        let two = listen().await;
        h.sighted("hub", vec![at(&one), at(&two), DEAD_ADDR]);

        let started = Instant::now();
        let first = h.next().await;
        assert_eq!(first["hub"].answered_at.len(), 1);
        assert!(started.elapsed() < PROBE_TIMEOUT);

        // The blackhole has to time out before the sweep can finish.
        let full = h.next().await;
        let mut answered = full["hub"].answered_at.clone();
        answered.sort();
        let mut expected = vec![at(&one), at(&two)];
        expected.sort();
        assert_eq!(answered, expected);
    }

    /// A hub answering where it answered before is not re-probed until
    /// [`REPROBE_INTERVAL`] has passed, so what it is published at holds.
    #[tokio::test]
    async fn the_first_address_holds_while_it_still_answers() {
        let mut h = Harness::new();
        let one = listen().await;
        let two = listen().await;
        h.sighted("hub", vec![at(&one), at(&two)]);

        let mut set = h.next().await;
        while set["hub"].answered_at.len() < 2 {
            set = h.next().await;
        }
        let first = set["hub"].answered_at[0];

        for _ in 0..5 {
            h.sighted("hub", vec![at(&one), at(&two)]);
        }
        h.unchanged().await;
        assert_eq!(h.hubs.borrow()["hub"].answered_at[0], first);
    }

    #[test]
    fn an_address_on_a_subnet_of_ours_is_published_before_one_that_is_not() {
        let subnets = [(Ipv4Addr::new(192, 168, 0, 105), 24)];
        let bridge = SocketAddr::from((Ipv4Addr::new(172, 17, 0, 1), 80));
        let lan = SocketAddr::from((Ipv4Addr::new(192, 168, 0, 7), 80));
        let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, 80));

        let mut answered = vec![loopback, bridge, lan];
        answered.sort_by_key(|addr| (hops_away(*addr, &subnets), *addr));

        assert_eq!(answered, [lan, bridge, loopback]);
    }

    /// A phone that rejoins its LAN sights the hub at the addresses it always
    /// advertised, and the probe from before the interface came up answered
    /// nowhere. Nothing about the sighting changed, so only the network change
    /// can be what lets it be probed again.
    #[tokio::test]
    async fn a_network_change_reprobes_a_hub_at_the_same_addresses() {
        let mut h = Harness::new();
        // The address the hub is on either way; it only answers on it later.
        let addr = at(&listen().await);
        h.seen("hub", addr);
        // Well inside `REPROBE_INTERVAL`, so only the network change can be
        // what lets the second sighting through.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // These run in parallel: another test can take the port in between,
        // and then there is nothing here to prove.
        let Ok(_hub) = TcpListener::bind(addr).await else {
            return;
        };
        h.browser.forget_probes_that_cannot_answer();
        h.seen("hub", addr);

        let started = Instant::now();
        assert_eq!(h.next().await, hubs(&[("hub", addr)]));
        assert!(started.elapsed() < REPROBE_INTERVAL);
    }

    /// A sighting routinely arrives before the network change behind it is
    /// noticed, so the probe it started is dialling the network we are on and
    /// is the fastest answer there is; only one dialling elsewhere is a probe
    /// the rejoin has to take the gate back from.
    #[tokio::test]
    async fn a_rejoin_forgets_only_the_probes_dialling_elsewhere() {
        fn probing(addr: SocketAddr) -> Hub {
            Hub {
                probed_addrs: [addr].into_iter().collect(),
                awaiting_probe: Some(1),
                probed_at: Some(Instant::now()),
                ..Default::default()
            }
        }

        let h = Harness::new();
        let here = at(&listen().await);
        {
            let mut hubs = h.browser.hubs.lock().unwrap();
            hubs.insert("here".to_string(), probing(here));
            hubs.insert("elsewhere".to_string(), probing(DEAD_ADDR));
        }

        h.browser.forget_probes_that_cannot_answer();

        let hubs = h.browser.hubs.lock().unwrap();
        assert_eq!(hubs["here"].awaiting_probe, Some(1));
        assert_eq!(hubs["elsewhere"].awaiting_probe, None);
        assert!(hubs["here"].probed_at.is_none());
        assert!(hubs["elsewhere"].probed_at.is_none());
    }

    #[tokio::test]
    async fn a_hub_that_never_answers_never_joins_the_set() {
        let mut h = Harness::new();
        h.seen("dead", DEAD_ADDR);
        h.unchanged().await;
    }

    #[tokio::test]
    async fn an_unreachable_hub_does_not_delay_a_reachable_one() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("dead", DEAD_ADDR);
        h.seen("hub", hub_addr);

        let started = Instant::now();
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));
        assert!(started.elapsed() < PROBE_TIMEOUT);
    }

    #[tokio::test]
    async fn a_hub_sighted_at_new_addresses_moves_in_the_set() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));

        let moved = listen().await;
        let moved_addr = moved.local_addr().unwrap();
        h.seen("hub", moved_addr);
        assert_eq!(h.next().await, hubs(&[("hub", moved_addr)]));
    }

    #[tokio::test]
    async fn a_hub_that_stops_answering_stays_until_its_announcements_lapse() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));

        drop(hub);
        h.seen("hub", hub_addr);
        h.unchanged().await;

        h.expired("hub");
        assert_eq!(h.next().await, BTreeMap::new());
    }

    #[tokio::test]
    async fn a_hub_that_says_goodbye_leaves_the_set_at_once() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));

        // Still listening, so only the goodbye can take it out.
        let started = Instant::now();
        h.said_goodbye("hub", hub_addr);
        assert_eq!(h.next().await, BTreeMap::new());
        assert!(started.elapsed() < PROBE_TIMEOUT);
    }

    #[tokio::test]
    async fn a_goodbye_from_a_hub_that_never_answered_changes_nothing() {
        let mut h = Harness::new();
        let hub = listen().await;
        h.said_goodbye("hub", hub.local_addr().unwrap());
        h.unchanged().await;
    }

    #[tokio::test]
    async fn a_probe_still_running_when_a_hub_left_does_not_put_it_back() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        // The probe of this sighting is still in flight (the blackhole holds it
        // for PROBE_TIMEOUT) when the goodbye arrives and is acted on.
        h.sighted("hub", vec![DEAD_ADDR, hub_addr]);
        h.said_goodbye("hub", hub_addr);
        h.unchanged().await;
    }

    #[tokio::test]
    async fn an_expired_hub_leaves_the_set() {
        let mut h = Harness::new();
        let hub = listen().await;
        let hub_addr = hub.local_addr().unwrap();
        h.seen("hub", hub_addr);
        assert_eq!(h.next().await, hubs(&[("hub", hub_addr)]));

        h.expired("hub");
        assert_eq!(h.next().await, BTreeMap::new());
    }

    #[tokio::test]
    async fn hubs_come_and_go_independently() {
        let mut h = Harness::new();
        let survivor = listen().await;
        let survivor_addr = survivor.local_addr().unwrap();
        let leaving = listen().await;
        let leaving_addr = leaving.local_addr().unwrap();
        h.seen("survivor", survivor_addr);
        h.seen("leaving", leaving_addr);
        let mut set = h.next().await;
        while set.len() < 2 {
            set = h.next().await;
        }
        assert_eq!(
            set,
            hubs(&[("survivor", survivor_addr), ("leaving", leaving_addr)])
        );

        h.said_goodbye("leaving", leaving_addr);
        assert_eq!(h.next().await, hubs(&[("survivor", survivor_addr)]));
    }
}
