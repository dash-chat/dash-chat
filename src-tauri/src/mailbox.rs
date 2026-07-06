use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio_util::task::AbortOnDropHandle;

// In e2e mode, use a distinct service type so test agents only discover each
// other's local mailboxes, not external dash-chat instances on the same LAN
// (which would otherwise show up as a connected "local" mailbox and break
// offline-UX assertions).
#[cfg(not(feature = "e2e-tests"))]
const MDNS_SERVICE_TYPE: &str = mailbox_mdns_discovery::MAILBOX_SERVICE_TYPE;
#[cfg(feature = "e2e-tests")]
const MDNS_SERVICE_TYPE: &str = "_dashchat-e2e._tcp.local.";
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

/// Browse the LAN for local mailboxes (via the shared `mailbox-mdns-discovery`
/// crate) and register/unregister each reachable peer directly into the node's
/// mailbox manager. The browse/probe/interface-watch and registration loop all
/// live in that crate, shared with the headless `local-mailbox-server`.
pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let mdns: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();
    log::info!("Started mdns browse for local mailboxes: {MDNS_SERVICE_TYPE}");
    let on_registered = {
        let node = node.clone();
        move |mailbox_id: mailbox_client::MailboxId, url: String| {
            let node = node.clone();
            async move {
                // Add the mailbox's dialing address to the address book so
                // the blob downloader can reach it by EndpointId rather than
                // relying solely on p2panda mDNS resolution timing.
                match mailbox_client::fetch_mailbox_health(&url).await {
                    Ok(health) => {
                        if let Err(err) = node.insert_peer_addr(health.endpoint_addr).await {
                            log::warn!(
                                "Failed to add local mailbox {mailbox_id} addr to address book: {err}"
                            );
                        }
                    }
                    Err(err) => log::warn!(
                        "Failed to fetch local mailbox {mailbox_id} health for address book: {err}"
                    ),
                }
                // Tell the mailbox our own dialing address so its blob fetch
                // pool can reach us as a source.
                // NOTE: on network changes, mDNS re-browse fires a new
                // ServiceResolved for each known mailbox, which re-runs this
                // path and re-registers the updated EndpointAddr. Cloud
                // mailboxes don't have this hook; re-registration there would
                // require a network-change callback from the node layer.
                if let Err(err) = node.register_with_mailbox(&url).await {
                    log::warn!(
                        "Failed to register our addr with local mailbox {mailbox_id}: {err}"
                    );
                }
                log::info!(
                    "*** Registered local mailbox client via mdns: {mailbox_id} ({url}) ***",
                );
            }
        }
    };
    let task = tokio::spawn(mailbox_mdns_discovery::discover_mailboxes_loop(
        mdns,
        MDNS_SERVICE_TYPE.to_string(),
        node.mailboxes.clone(),
        node.endpoint_id(),
        node.unfetched_blob_tracker(),
        on_registered,
    ));
    Ok(AbortOnDropHandle::new(task))
}
