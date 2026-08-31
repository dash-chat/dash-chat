use local_hub_discovery::{spawn_local_hub_discovery, DiscoveredHub, LocalHubEvent};
use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio_util::task::AbortOnDropHandle;

#[cfg(not(feature = "e2e-tests"))]
const MDNS_SERVICE_TYPE: &str = local_hub_discovery::MDNS_SERVICE_TYPE;
#[cfg(feature = "e2e-tests")]
const MDNS_SERVICE_TYPE: &str = local_hub_discovery::E2E_MDNS_SERVICE_TYPE;
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

/// Keep the node's mailbox manager in step with the local hubs on the LAN.
///
/// Discovery itself — the mDNS browse, its re-arm on network changes, and the
/// reachability probing — lives in `local-hub-discovery`. What remains here is
/// only the node-side policy for what a discovered hub means.
pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let mdns: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();
    let mut discovery = spawn_local_hub_discovery(mdns, MDNS_SERVICE_TYPE)?;

    let handler_task = tokio::spawn(async move {
        while let Some(event) = discovery.recv().await {
            match event {
                LocalHubEvent::Found(hub) => register_local_hub(&node, hub).await,
                LocalHubEvent::Lost { id } => {
                    if node.mailboxes.unregister(&id).await {
                        log::info!("*** Removed local mailbox client via mdns: {id} ***");
                    }
                }
            }
        }
    });

    Ok(AbortOnDropHandle::new(handler_task))
}

/// Point the node at a hub we just found: register it as a mailbox, learn its
/// dialing address, and hand it ours.
///
/// Safe to re-run — `MailboxManager::register` swaps the client in place — which
/// matters because every re-browse re-resolves the hubs it already knows.
async fn register_local_hub(node: &dashchat_node::Node, hub: DiscoveredHub) {
    let DiscoveredHub { id, url } = hub;
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
    // Tell the hub our own dialing address so its blob fetch pool can reach us as
    // a source. A re-browse re-resolves every known hub, so this also refreshes
    // the EndpointAddr after a network change. Cloud mailboxes have no such hook;
    // refreshing there would need a network-change callback from the node layer.
    if let Err(err) = node.register_with_mailbox(&url).await {
        log::warn!("Failed to register our addr with local mailbox {id}: {err}");
    }
    log::info!("*** Registered local mailbox client via mdns: {id} ({url}) ***");
}
