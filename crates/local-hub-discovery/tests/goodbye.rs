//! Both halves of the goodbye in one process: what [`LocalHubAnnouncementService`]
//! sets on its way out is what [`LocalHubDiscoveryService`] reads as a hub
//! leaving, and it goes out while the service is still up to send it.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use data_encoding::BASE64URL_NOPAD;
use local_hub_discovery::{DiscoveredHub, LocalHubAnnouncementService, LocalHubDiscoveryService};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Longer than one announce round, shorter than the ~2.4s a swarm-discovery
/// peer takes to lapse at the interactive cadence: a hub gone within this left
/// because of its goodbye, not because its announcements ran out.
const GOODBYE_WITHIN: Duration = Duration::from_secs(2);
const DISCOVERY_WITHIN: Duration = Duration::from_secs(4);

/// A hub id of the shape the announce side encodes: base64url, as a MailboxId
/// is. Unique per process, so two test runs on one LAN cannot see each other's.
fn hub_id() -> String {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&std::process::id().to_le_bytes());
    bytes[4..12].copy_from_slice(&Instant::now().elapsed().as_nanos().to_le_bytes()[..8]);
    BASE64URL_NOPAD.encode(&bytes)
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

    let said_goodbye = Instant::now();
    announcement.shutdown().await;

    wait_until(
        &mut hubs,
        GOODBYE_WITHIN,
        "the hub that said goodbye never left the published set",
        |published| !published.contains_key(&id),
    )
    .await;
    assert!(said_goodbye.elapsed() < GOODBYE_WITHIN);
}
