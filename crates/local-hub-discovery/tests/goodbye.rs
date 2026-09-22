//! Both halves of the goodbye in one process: what [`LocalHubAnnouncementService`]
//! sets on its way out is what [`LocalHubDiscoveryService`] reads as a hub
//! leaving, and it goes out while the service is still up to send it.
//!
//! Needs working IPv4 multicast on the host: the two halves meet over mDNS, so
//! somewhere with only loopback (a container, Linux `lo` carries no MULTICAST
//! flag) they never see each other and this fails as a discovery timeout.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use data_encoding::BASE64URL_NOPAD;
use local_hub_discovery::{DiscoveredHub, LocalHubAnnouncementService, LocalHubDiscoveryService};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Measured from when `shutdown` returns, by which point the goodbye has had
/// its full round
const GOODBYE_WITHIN: Duration = Duration::from_millis(1_000);
const DISCOVERY_WITHIN: Duration = Duration::from_secs(4);

/// A hub id of the shape the announce side encodes: base64url, as a MailboxId
/// is. Wall-clock nanos and the pid, so two runs of this test never collide.
fn hub_id() -> String {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&std::process::id().to_le_bytes());
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    bytes[4..12].copy_from_slice(&(since_epoch.as_nanos() as u64).to_le_bytes());
    BASE64URL_NOPAD.encode(&bytes)
}

/// mDNS needs an interface that carries multicast: somewhere with only loopback
/// (Linux `lo` has no MULTICAST flag) the two halves can never meet, and this
/// would fail as a discovery timeout rather than say why.
fn multicast_available() -> bool {
    if_addrs::get_if_addrs().is_ok_and(|interfaces| {
        interfaces
            .iter()
            .any(|interface| !interface.is_loopback() && interface.ip().is_ipv4())
    })
}

/// A port that answers a probe, so the hub reaches the published set at all.
async fn listening_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { while listener.accept().await.is_ok() {} });
    port
}

async fn wait_until(
    hubs: &mut watch::Receiver<BTreeMap<String, DiscoveredHub>>,
    within: Duration,
    what: &str,
    mut done: impl FnMut(&BTreeMap<String, DiscoveredHub>) -> bool,
) {
    let deadline = Instant::now() + within;
    loop {
        if done(&hubs.borrow_and_update()) {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || tokio::time::timeout(left, hubs.changed()).await.is_err() {
            panic!("{what} within {within:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_says_goodbye_leaves_the_set_before_its_announcements_lapse() {
    if !multicast_available() {
        eprintln!("skipped: no multicast-capable IPv4 interface on this host");
        return;
    }
    // Keeps the announcement off the name real clients browse. Set before
    // anything resolves `service_name`, which caches on first use.
    std::env::set_var(
        "LOCAL_HUB_SERVICE_ID",
        format!("test{}", std::process::id()),
    );
    let id = hub_id();
    let port = listening_port().await;

    let discovery = LocalHubDiscoveryService::spawn();
    let mut hubs = discovery.hubs();
    let announcement = LocalHubAnnouncementService::spawn(&id, port).unwrap();

    wait_until(
        &mut hubs,
        DISCOVERY_WITHIN,
        "the announced hub never reached the published set",
        |published| published.contains_key(&id),
    )
    .await;

    announcement.shutdown().await;

    wait_until(
        &mut hubs,
        GOODBYE_WITHIN,
        "the hub that said goodbye never left the published set",
        |published| !published.contains_key(&id),
    )
    .await;
}
