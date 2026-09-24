use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use local_hub_discovery::{DiscoveredHub, LocalHubDiscoveryService};
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;

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

/// Keep the node's mailbox manager and address book in step with the local
/// hubs on the LAN.
pub fn spawn_local_mailbox_mdns_discovery(
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let discovery = LocalHubDiscoveryService::spawn(local_hub_discovery::service_name());
    let hubs = discovery.hubs();

    let handler_task = tokio::spawn(async move {
        let _discovery = discovery;
        register_local_hubs(node, hubs).await;
    });

    Ok(AbortOnDropHandle::new(handler_task))
}

/// Register the hubs discovery publishes and drop the ones it stops publishing.
///
/// Discovery is the only authority on which hubs are there: letting how a
/// mailbox is faring decide what stays registered made a hub that answers TCP
/// while failing HTTP churn through register and unregister.
async fn register_local_hubs(
    node: dashchat_node::Node,
    mut hubs: watch::Receiver<BTreeMap<String, DiscoveredHub>>,
) {
    let mut ours: BTreeSet<String> = BTreeSet::new();
    let mut learning_addrs: BTreeMap<String, AbortOnDropHandle<()>> = BTreeMap::new();
    loop {
        let current = hubs.borrow_and_update().clone();
        reconcile(&node, &mut ours, &mut learning_addrs, &current).await;
        if hubs.changed().await.is_err() {
            return;
        }
    }
}

/// A hub the node already holds at the nearest address it answers on is left
/// alone; anywhere else it is registered again, so the sort decides where a hub
/// is polled rather than whichever address won the probe race.
async fn reconcile(
    node: &dashchat_node::Node,
    ours: &mut BTreeSet<String>,
    learning_addrs: &mut BTreeMap<String, AbortOnDropHandle<()>>,
    current: &BTreeMap<String, DiscoveredHub>,
) {
    for hub in current.values() {
        let Some(&addr) = hub.answered_at.first() else {
            continue;
        };
        ours.insert(hub.mailbox_id.clone());
        if registered_url(node, &hub.mailbox_id).await.as_deref() != Some(hub_url(addr).as_str()) {
            let learning = register_local_hub(node, hub, addr).await;
            learning_addrs.insert(hub.mailbox_id.clone(), learning);
        }
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
        learning_addrs.remove(&id);
    }
}

/// Where the node holds this hub's mailbox, if it holds one at all. A url, not
/// an address, so this and [`hub_url`] cannot drift apart silently.
async fn registered_url(node: &dashchat_node::Node, id: &str) -> Option<String> {
    let tracked = node.mailboxes.tracked_mailbox(&id.to_string()).await?;
    tracked.client().await.url()
}

fn hub_url(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

/// Safe to re-run — `MailboxManager::register` swaps the client in place —
/// which matters because [`reconcile`] runs this again whenever the address the
/// node holds is no longer the nearest one the hub answers on.
async fn register_local_hub(
    node: &dashchat_node::Node,
    hub: &DiscoveredHub,
    addr: SocketAddr,
) -> AbortOnDropHandle<()> {
    let id = &hub.mailbox_id;
    let url = hub_url(addr);
    node.mailboxes
        .register(mailbox_client::toy::ToyMailboxClient::new(
            id.clone(),
            url.clone(),
            node.endpoint_id(),
        ))
        .await;
    log::info!("*** Registered local mailbox client via mdns: {id} ({url}) ***");
    // Spawned: the registering loop must not wait on a hub that has just left
    // the LAN and takes seconds to fail.
    AbortOnDropHandle::new(tokio::spawn(learn_hub_addr(node.clone(), url)))
}

/// Put the hub's dialing address in the address book, so blobs can be pushed
/// to and downloaded from it without waiting on p2panda mDNS resolution.
async fn learn_hub_addr(node: dashchat_node::Node, url: String) {
    let _ = dashchat_utils::retry_with_backoff(
        None,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(10),
        "learn local mailbox address",
        || async {
            let health = dashchat_node::mailbox::fetch_mailbox_health(&url).await?;
            node.insert_peer_addr(health.endpoint_addr).await
        },
    )
    .await;
}
