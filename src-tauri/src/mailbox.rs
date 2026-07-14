use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};
use tokio_util::task::AbortOnDropHandle;

// In e2e mode, use a distinct service type so test agents only discover each
// other's local mailboxes, not external dash-chat instances on the same LAN
// (which would otherwise show up as a connected "local" mailbox and break
// offline-UX assertions).
#[cfg(not(feature = "e2e-tests"))]
const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";
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

pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> anyhow::Result<AbortOnDropHandle<()>> {
    let mdns: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();
    let receiver = mdns.browse(MDNS_SERVICE_TYPE)?;
    log::info!("Started mdns browse for local mailboxes: {MDNS_SERVICE_TYPE}");

    // Interface changes are handled by the mdns-sd daemon itself: it re-checks
    // interfaces periodically and re-sends browse queries on new ones, so the
    // browse doesn't need to be re-issued on network switches.
    let handler_task = tokio::spawn(handle_browse_events(node, receiver));

    Ok(AbortOnDropHandle::new(handler_task))
}

async fn handle_browse_events(
    node: dashchat_node::Node,
    receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>,
) {
    while let Ok(event) = receiver.recv_async().await {
        match event {
            mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
                // Use the announcer's NODEID-derived instance name as the
                // mailbox id, not the raw mDNS fullname. The fullname carries
                // the service-type suffix (e.g. `._dashchat._tcp.local.`) and,
                // when an mDNS daemon (Apple Bonjour in particular) detects a
                // perceived registration conflict with a stale cached entry,
                // it appends a ` (N)` uniqueness counter. Both are noise from
                // the application's perspective — what we want is the stable
                // node identifier the announcer chose.
                let mailbox_id =
                    instance_name_from_fullname(&resolved.fullname, &resolved.ty_domain);
                let port = resolved.port;
                let node = node.clone();

                // The local mailbox server listens dual-stack (`[::]:port`), so
                // both IPv4 and IPv6 records are usable. We probe each address
                // before registering. Non-loopback first (more stable identity
                // in logs, works regardless of which side issued the
                // announcement), then loopback as a fallback. A loopback
                // address from a *remote* announcer would point at our own
                // loopback — but the probe will fail unless we happen to be
                // listening on that exact port, which for a randomly-allocated
                // mailbox server port is vanishingly unlikely. For our *own*
                // announcement, the loopback fallback is what makes offline
                // self-discovery work.
                //
                // Probe + register runs off the handler so a slow / unreachable
                // peer can't stall the event loop. Re-resolutions for the same
                // mailbox_id are idempotent: `MailboxManager::register` swaps
                // the client in place via `replace_client`.
                tokio::spawn(async move {
                    let Some(host) = pick_reachable_host(&resolved, port).await else {
                        log::info!(
                            "Resolved mdns service {mailbox_id}: no address in the announcement is reachable on port {port}, waiting for next announcement"
                        );
                        return;
                    };

                    let url = format!("http://{host}:{port}");
                    node.mailboxes
                        .register(
                            mailbox_client::toy::ToyMailboxClient::new(
                                mailbox_id.clone(),
                                url.clone(),
                                node.endpoint_id(),
                                node.unfetched_blob_tracker(),
                            )
                            .with_blob_reader(node.blob_reader()),
                        )
                        .await;
                    // Add the mailbox's dialing address to the address book so
                    // the blob downloader can reach it by EndpointId rather than
                    // relying solely on p2panda mDNS resolution timing.
                    match dashchat_node::mailbox::fetch_mailbox_health(&url).await {
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
                });
            }
            mdns_sd::ServiceEvent::ServiceRemoved(ty_domain, fullname) => {
                let mailbox_id = instance_name_from_fullname(&fullname, &ty_domain);
                if node.mailboxes.unregister(&mailbox_id).await {
                    log::info!("*** Removed local mailbox client via mdns: {mailbox_id} ***");
                }
            }
            other_event => {
                log::trace!("((( Received other mdns event: {:?} )))", &other_event);
            }
        }
    }

    log::debug!("mdns browse handler loop ended");
}

/// Per-address TCP probe budget. Probes run concurrently within a tier, so the
/// effective per-event latency is closer to the slowest single RTT than to the
/// sum. Generous enough to give a sleepy mobile peer time to ACK over lossy
/// Wi-Fi — a tight (e.g. 500 ms) bound risks skipping otherwise-reachable
/// peers whenever the first SYN gets dropped and has to retransmit.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Concurrently TCP-probe the routable addresses, then the loopback addresses,
/// returning the first that accepts a connection within `PROBE_TIMEOUT`,
/// formatted for a URL authority (IPv6 wrapped in `[...]`). Returns `None` if
/// no address in the announcement is reachable on the given port. The probe is
/// a bare TCP connect — it doesn't verify the listener is actually a mailbox
/// server, but in practice no other service shares the randomly-allocated port.
async fn pick_reachable_host(resolved: &mdns_sd::ResolvedService, port: u16) -> Option<String> {
    let ips = resolved.addresses.iter().filter_map(|addr| match addr {
        mdns_sd::ScopedIp::V4(ip) => Some(std::net::IpAddr::V4(*ip.addr())),
        // Link-local IPv6 (fe80::/10) needs a `%ifN` zone identifier to route;
        // the announcer's interface index isn't usable from here, so skip it.
        mdns_sd::ScopedIp::V6(ip) if !ip.addr().is_unicast_link_local() => {
            Some(std::net::IpAddr::V6(*ip.addr()))
        }
        _ => None,
    });
    let (loopback, routable): (Vec<_>, Vec<_>) = ips.partition(|ip| ip.is_loopback());
    if let Some(host) = probe_first_reachable(&routable, port).await {
        return Some(host);
    }
    probe_first_reachable(&loopback, port).await
}

async fn probe_first_reachable(ips: &[std::net::IpAddr], port: u16) -> Option<String> {
    if ips.is_empty() {
        return None;
    }
    let probes = ips.iter().map(|&ip| {
        Box::pin(async move {
            if probe_tcp(ip, port).await {
                Ok::<std::net::IpAddr, ()>(ip)
            } else {
                Err(())
            }
        })
    });
    futures::future::select_ok(probes)
        .await
        .ok()
        .map(|(ip, _)| url_host(ip))
}

fn url_host(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V4(ip) => ip.to_string(),
        std::net::IpAddr::V6(ip) => format!("[{ip}]"),
    }
}

async fn probe_tcp(host: std::net::IpAddr, port: u16) -> bool {
    let addr = std::net::SocketAddr::from((host, port));
    let connect = tokio::net::TcpStream::connect(&addr);
    match tokio::time::timeout(PROBE_TIMEOUT, connect).await {
        Ok(Ok(_stream)) => true,
        Ok(Err(err)) => {
            log::trace!("Probe to {addr} refused / errored: {err}");
            false
        }
        Err(_elapsed) => {
            log::trace!("Probe to {addr} timed out after {:?}", PROBE_TIMEOUT);
            false
        }
    }
}

/// Recover the announcer-chosen instance name from a resolved mDNS fullname.
///
/// The fullname is shaped as `<instance>.<ty_domain>` (e.g.
/// `"f73abc...._dashchat._tcp.local."`). When the local mDNS daemon detects a
/// registration conflict (typically a stale cached entry from a previous
/// session), it appends a `" (N)"` uniqueness counter to the instance name —
/// so the fullname can come back as `"f73abc... (2)._dashchat._tcp.local."`.
/// Both the type suffix and the counter are mDNS plumbing the application
/// shouldn't see; strip them so the mailbox id stays the stable NODEID the
/// announcer set.
fn instance_name_from_fullname(fullname: &str, ty_domain: &str) -> String {
    let instance = fullname
        .strip_suffix(ty_domain)
        .and_then(|s| s.strip_suffix('.'))
        .unwrap_or(fullname);
    strip_collision_suffix(instance).to_string()
}

fn strip_collision_suffix(name: &str) -> &str {
    let Some(stripped) = name.strip_suffix(')') else {
        return name;
    };
    let Some(open_idx) = stripped.rfind(" (") else {
        return name;
    };
    let inner = &stripped[open_idx + 2..];
    if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
        &name[..open_idx]
    } else {
        name
    }
}
