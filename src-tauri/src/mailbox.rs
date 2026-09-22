use std::collections::BTreeMap;
use std::net::SocketAddr;

use local_hub_discovery::{DiscoveredHub, LocalHubDiscoveryService};
use tokio::sync::broadcast;
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

/// Keep the node's mailbox manager in step with the local hubs on the LAN.
pub fn spawn_local_mailbox_mdns_discovery(
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let discovery = LocalHubDiscoveryService::spawn();
    let mut hubs = discovery.hubs();
    let mut network = network_watch::network_change();

    let handler_task = tokio::spawn(async move {
        let _discovery = discovery;
        let mut registered: BTreeMap<String, SocketAddr> = BTreeMap::new();
        loop {
            let current = hubs.borrow_and_update().clone();
            reconcile(&node, &mut registered, &current).await;
            tokio::select! {
                stopped = hubs.changed() => {
                    if stopped.is_err() {
                        return;
                    }
                }
                // A hub cannot tell us our address changed, so re-register with
                // every one of them to hand it over again.
                changed = network.recv() => {
                    if matches!(changed, Err(broadcast::error::RecvError::Closed)) {
                        return;
                    }
                    registered.clear();
                }
            }
        }
    });

    Ok(AbortOnDropHandle::new(handler_task))
}

/// Register the hubs that appeared, register anew the ones whose address we
/// hold stopped answering, and drop the ones that went. A hub that is still
/// answering where we registered it is left alone, so the other addresses it
/// answers at coming and going cannot churn a working registration.
async fn reconcile(
    node: &dashchat_node::Node,
    registered: &mut BTreeMap<String, SocketAddr>,
    current: &BTreeMap<String, DiscoveredHub>,
) {
    for (id, hub) in current {
        if registered
            .get(id)
            .is_some_and(|at| hub.answered_at.contains(at))
        {
            continue;
        }
        let Some(&addr) = hub.answered_at.first() else {
            continue;
        };
        register_local_hub(node, hub, addr).await;
        registered.insert(id.clone(), addr);
    }
    let gone: Vec<String> = registered
        .keys()
        .filter(|id| !current.contains_key(*id))
        .cloned()
        .collect();
    for id in gone {
        if node.mailboxes.unregister(&id).await {
            log::info!("*** Removed local mailbox client via mdns: {id} ***");
        }
        registered.remove(&id);
    }
}

/// Point the node at a hub: register it as a mailbox, learn its dialing
/// address, and hand it ours.
///
/// Safe to re-run — `MailboxManager::register` swaps the client in place —
/// which matters because [`reconcile`] runs this again whenever the address we
/// registered stops answering or the network changes.
async fn register_local_hub(node: &dashchat_node::Node, hub: &DiscoveredHub, addr: SocketAddr) {
    let id = &hub.mailbox_id;
    let url = format!("http://{addr}");
    let newly_tracked = !node.mailboxes.is_tracked(id).await;
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
    // it comes back. Only the first registration arms this, since a re-browse
    // re-registers every hub it still sees.
    if newly_tracked {
        node.mailboxes.unregister_on_stopped(id).await;
    }
    // Add the hub's dialing address to the address book so the blob downloader
    // can reach it by EndpointId rather than relying solely on p2panda mDNS
    // resolution timing.
    match dashchat_node::mailbox::fetch_mailbox_health(&url).await {
        Ok(health) => {
            if let Err(err) = node.insert_peer_addr(health.endpoint_addr).await {
                log::warn!("Failed to add local mailbox {id} addr to address book: {err}");
            }
        }
        Err(err) => {
            log::warn!("Failed to fetch local mailbox {id} health for address book: {err}")
        }
    }
    // Tell the hub our own dialing address so its blob fetch pool can reach us
    // as a source; the reconcile loop re-registers on every network change, so
    // this refreshes the EndpointAddr then too.
    if let Err(err) = node.register_with_mailbox(&url).await {
        log::warn!("Failed to register our addr with local mailbox {id}: {err}");
    }
    log::info!("*** Registered local mailbox client via mdns: {id} ({url}) ***");
}
