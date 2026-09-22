use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use local_hub_discovery::{DiscoveredHub, LocalHubDiscoveryService};
use tokio::sync::{broadcast, watch};
use tokio_util::task::AbortOnDropHandle;

/// How long a hub that did not take our address waits before we try again.
const HAND_OVER_RETRY: std::time::Duration = std::time::Duration::from_secs(5);

pub(crate) const PRODUCTION_MAILBOX_URL: &str = "https://mailbox.production.darksoil.studio";

#[cfg(not(mobile))]
pub mod server;

/// Returns the mailbox URL to use.
///
/// Resolution order:
/// 1. `MAILBOX_URL` runtime env var (E2E tests)
/// 2. `MAILBOX_URL` compile-time env var (set by build.rs in debug builds)
/// 3. Production URL
pub fn default_mailbox_url() -> String {
    if let Ok(url) = std::env::var("MAILBOX_URL") {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            log::error!(
                "MAILBOX_URL env var is not a valid URL: {url}, falling back to next option"
            );
        } else {
            return url;
        }
    }
    if let Some(url) = option_env!("MAILBOX_URL") {
        log::info!("Using compile-time MAILBOX_URL: {url}");
        return url.to_string();
    }
    PRODUCTION_MAILBOX_URL.to_string()
}

/// The id of the mailbox whose URL is the cloud URL, if any.
///
/// "Cloud" is an app-level concept — the generic `Mailboxes` manager has no
/// notion of it — so we identify it by matching `default_mailbox_url()` against
/// each registered mailbox's client URL. When no registered mailbox matches
/// (e.g. after a cold start while the cloud server is unreachable, so it can't
/// be re-registered), we fall back to the URL persisted in the sync tracker so
/// a previously-delivered message still resolves to the cloud mailbox. Returns
/// `None` only when the cloud mailbox has never been reached on this device.
pub(crate) async fn cloud_mailbox_id(
    node: &dashchat_node::Node,
) -> Option<mailbox_client::MailboxId> {
    let cloud_url = default_mailbox_url();
    let ids = node.mailboxes.active_mailbox_ids().borrow().clone();
    for id in ids {
        if let Some(tm) = node.mailboxes.tracked_mailbox(&id).await {
            if tm.client().await.url().as_deref() == Some(&cloud_url) {
                return Some(id);
            }
        }
    }
    node.mailboxes
        .sync_tracker()
        .mailbox_id_for_url(&cloud_url)
        .await
        .unwrap_or(None)
}

/// Poll the cloud mailbox now without presuming the result: one success
/// restores Active, one failure only confirms an existing backoff. Falls back
/// to nudging the poll loop when the cloud mailbox has never been reached.
pub(crate) async fn probe_cloud_mailbox(node: &dashchat_node::Node) {
    match cloud_mailbox_id(node).await {
        Some(cloud_id) => node.mailboxes.probe(cloud_id).await,
        None => node.mailboxes.nudge_poll_loop(),
    }
}

/// Keep the node's mailbox manager in step with the local hubs on the LAN, and
/// keep those hubs holding our dialing address.
pub fn spawn_local_mailbox_mdns_discovery(
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let discovery = LocalHubDiscoveryService::spawn(local_hub_discovery::service_name());
    let hubs = discovery.hubs();

    let handler_task = tokio::spawn(async move {
        let _discovery = discovery;
        let _handing_over =
            AbortOnDropHandle::new(tokio::spawn(hand_over_our_addr(node.clone(), hubs.clone())));
        register_local_hubs(node, hubs).await;
    });

    Ok(AbortOnDropHandle::new(handler_task))
}

/// Register the hubs discovery publishes and drop the ones it stops publishing,
/// for as long as either the hubs or the node's own mailboxes change.
async fn register_local_hubs(
    node: dashchat_node::Node,
    mut hubs: watch::Receiver<BTreeMap<String, DiscoveredHub>>,
) {
    let mut tracked = node.mailboxes.active_mailbox_ids();
    let mut ours: BTreeSet<String> = BTreeSet::new();
    loop {
        let current = hubs.borrow_and_update().clone();
        reconcile(&node, &mut ours, &current).await;
        tokio::select! {
            stopped = hubs.changed() => {
                if stopped.is_err() {
                    return;
                }
            }
            // The node drops a hub's mailbox on its own once it stops answering
            // (`unregister_on_stopped`), and putting one back is this loop's
            // job — so it watches what the node holds, not what it last did.
            stopped = tracked.changed() => {
                if stopped.is_err() {
                    return;
                }
            }
        }
    }
}

/// A hub the node already holds at the nearest address it answers on is left
/// alone; anywhere else it is registered again, so the sort decides where a hub
/// is polled rather than whichever address won the probe race.
async fn reconcile(
    node: &dashchat_node::Node,
    ours: &mut BTreeSet<String>,
    current: &BTreeMap<String, DiscoveredHub>,
) {
    for hub in current.values() {
        let Some(&addr) = hub.answered_at.first() else {
            continue;
        };
        ours.insert(hub.mailbox_id.clone());
        if registered_url(node, hub).await.as_deref() == Some(hub_url(addr).as_str()) {
            // Ours already, but a mailbox that backed off while it was away
            // reads as disconnected until its next poll.
            node.mailboxes.probe(hub.mailbox_id.clone()).await;
            continue;
        }
        register_local_hub(node, hub, addr).await;
    }
    // Only what this loop put there: inferring it from the node's mailboxes
    // would tear down anything else that ever registers one.
    let gone: Vec<String> = ours
        .iter()
        .filter(|id| !current.contains_key(*id))
        .cloned()
        .collect();
    for id in gone {
        if node.mailboxes.unregister(&id).await {
            log::info!("*** Removed local mailbox client via mdns: {id} ***");
        }
        ours.remove(&id);
    }
}

/// Where the node holds this hub's mailbox, if it holds one at all. A url, not
/// an address, so this and [`hub_url`] cannot drift apart silently.
async fn registered_url(node: &dashchat_node::Node, hub: &DiscoveredHub) -> Option<String> {
    let tracked = node.mailboxes.tracked_mailbox(&hub.mailbox_id).await?;
    tracked.client().await.url()
}

fn hub_url(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

/// Safe to re-run — `MailboxManager::register` swaps the client in place —
/// which matters because [`reconcile`] runs this again whenever the address the
/// node holds stops answering, or the node drops the mailbox.
async fn register_local_hub(node: &dashchat_node::Node, hub: &DiscoveredHub, addr: SocketAddr) {
    let id = &hub.mailbox_id;
    let url = hub_url(addr);
    node.mailboxes
        .register(
            mailbox_client::toy::ToyMailboxClient::new(
                id.clone(),
                url.clone(),
                node.endpoint_id(),
                node.unfetched_blob_tracker(),
            )
            .with_blob_reader(node.blob_reader()),
        )
        .await;
    // A hub that stops answering is gone as far as we are concerned, whether or
    // not its mDNS records have expired yet; the re-browse re-registers it if
    // it comes back. Armed every time: reading whether it is already armed and
    // then registering leaves a window where the watcher fires in between and
    // the fresh mailbox is left with none.
    node.mailboxes.unregister_on_stopped(id).await;
    log::info!("*** Registered local mailbox client via mdns: {id} ({url}) ***");
}

/// Hand each hub our dialing address, and hand it over again whenever ours
/// changes: a hub cannot tell us that it moved, and it needs the new one to
/// reach us as a blob source.
///
/// Off the registering loop, which must not wait on a hub that has just left
/// the LAN and takes seconds to fail.
async fn hand_over_our_addr(
    node: dashchat_node::Node,
    mut hubs: watch::Receiver<BTreeMap<String, DiscoveredHub>>,
) {
    let mut network = network_watch::network_change();
    let mut handed_over: BTreeMap<String, SocketAddr> = BTreeMap::new();
    loop {
        let current = hubs.borrow_and_update().clone();
        let due: Vec<(String, SocketAddr)> = current
            .values()
            .filter_map(|hub| {
                let &addr = hub.answered_at.first()?;
                (handed_over.get(&hub.mailbox_id) != Some(&addr))
                    .then(|| (hub.mailbox_id.clone(), addr))
            })
            .collect();
        // Together, not in turn: a hub that answers a probe but stalls on HTTP
        // would otherwise hold up every hub behind it for two 10s timeouts.
        let done = futures::future::join_all(due.into_iter().map(|(id, addr)| {
            let node = &node;
            async move { exchange_addrs(node, &id, addr).await.then_some((id, addr)) }
        }))
        .await;
        // Only what succeeded: a hub wrongly recorded as told would never be
        // told again.
        handed_over.extend(done.into_iter().flatten());
        handed_over.retain(|id, _| current.contains_key(id));
        // A hub that keeps answering where it always did publishes no change, so
        // without this the loop would never wake to try a failed one again.
        let outstanding = current.values().any(|hub| {
            hub.answered_at
                .first()
                .is_some_and(|addr| handed_over.get(&hub.mailbox_id) != Some(addr))
        });
        tokio::select! {
            stopped = hubs.changed() => {
                if stopped.is_err() {
                    return;
                }
            }
            changed = network.recv() => {
                if matches!(changed, Err(broadcast::error::RecvError::Closed)) {
                    return;
                }
                handed_over.clear();
            }
            _ = tokio::time::sleep(HAND_OVER_RETRY), if outstanding => {}
        }
    }
}

/// Learn the hub's dialing address for the address book and hand it ours, so
/// blobs can move either way without waiting on p2panda mDNS resolution.
///
/// Reports whether the hub now holds our address, so a failed exchange is tried
/// again rather than recorded as done.
async fn exchange_addrs(node: &dashchat_node::Node, id: &str, addr: SocketAddr) -> bool {
    let url = hub_url(addr);
    let learnt = match dashchat_node::mailbox::fetch_mailbox_health(&url).await {
        Ok(health) => match node.insert_peer_addr(health.endpoint_addr).await {
            Ok(()) => true,
            Err(err) => {
                log::warn!("Failed to add local mailbox {id} addr to address book: {err}");
                false
            }
        },
        Err(err) => {
            log::warn!("Failed to fetch local mailbox {id} health for address book: {err}");
            false
        }
    };
    let handed_over = match node.register_with_mailbox(&url).await {
        Ok(()) => true,
        Err(err) => {
            log::warn!("Failed to register our addr with local mailbox {id}: {err}");
            false
        }
    };
    learnt && handed_over
}
