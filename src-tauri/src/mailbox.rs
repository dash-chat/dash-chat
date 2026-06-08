use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, Runtime};

// In e2e mode, use a distinct service type so test agents only discover each
// other's local mailboxes, not external dash-chat instances on the same LAN
// (which would otherwise show up as a connected "local" mailbox and break
// offline-UX assertions).
#[cfg(not(feature = "e2e-tests"))]
const MDNS_SERVICE_TYPE: &str = "_dashchat._tcp.local.";
#[cfg(feature = "e2e-tests")]
const MDNS_SERVICE_TYPE: &str = "_dashchat-e2e._tcp.local.";
pub(crate) const PRODUCTION_MAILBOX_ID: &str = "dashchat-mailbox";
pub(crate) const PRODUCTION_MAILBOX_URL: &str =
    "https://mailbox-server.production.dash-chat.dash-chat.garnix.me";

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

pub fn spawn_local_mailbox_mdns_discovery<R: Runtime>(
    handle: &AppHandle<R>,
    node: dashchat_node::Node,
) -> anyhow::Result<()> {
    let mdns: ServiceDaemon = handle.state::<ServiceDaemon>().inner().clone();
    let receiver = mdns.browse(MDNS_SERVICE_TYPE)?;
    log::info!("Started mdns browse for local mailboxes: {MDNS_SERVICE_TYPE}");

    let mut handler_task = tokio::spawn(handle_browse_events(node.clone(), receiver));

    // The browse receiver is tied to the interface set the daemon had at
    // `browse()` time; when the device switches Wi-Fi networks (or a new
    // interface like macOS's bridge100 from Internet Sharing appears later),
    // services on the new interface aren't picked up until we re-issue the
    // browse. Mirror what the server side does for re-announcement.
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut watcher = match if_watch::tokio::IfWatcher::new() {
            Ok(w) => w,
            Err(err) => {
                log::warn!("Failed to start mailbox browse interface watcher: {err:?}");
                return;
            }
        };

        while let Some(event) = watcher.next().await {
            match event {
                Ok(if_watch::IfEvent::Up(net)) => {
                    log::info!("Mailbox browse interface watcher: up {net}, refreshing mDNS browse");
                }
                Ok(if_watch::IfEvent::Down(net)) => {
                    log::info!("Mailbox browse interface watcher: down {net}, refreshing mDNS browse");
                }
                Err(err) => {
                    log::warn!("Mailbox browse interface watcher error: {err:?}");
                    continue;
                }
            }

            // Debounce 500ms so a burst of related interface events (a typical
            // Wi-Fi handoff produces several in quick succession) triggers a
            // single refresh.
            debounce_browse_burst(&mut watcher).await;

            // Tear down the current browse + handler, then restart against the
            // now-current interface set. `stop_browse` closes the existing
            // receiver, which exits the handler loop on its own; we also abort
            // it as a belt-and-suspenders guard in case stop_browse races with
            // an in-flight recv.
            if let Err(err) = mdns.stop_browse(MDNS_SERVICE_TYPE) {
                log::warn!("Failed to stop mDNS browse during refresh: {err:?}");
            }
            handler_task.abort();

            match mdns.browse(MDNS_SERVICE_TYPE) {
                Ok(receiver) => {
                    handler_task = tokio::spawn(handle_browse_events(node.clone(), receiver));
                }
                Err(err) => {
                    log::warn!("Failed to restart mDNS browse after interface change: {err:?}");
                }
            }
        }

        log::warn!("Mailbox browse interface watcher stream ended");
    });

    Ok(())
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

                // Prefer IPv4, fall back to non-link-local IPv6.
                // Link-local IPv6 (fe80::/10) needs a `%ifN` zone identifier to be
                // routable; the announcer's interface index isn't usable from here.
                // Without a zone, iOS TCP connect fails with EHOSTUNREACH.
                let host = resolved
                    .addresses
                    .iter()
                    .find_map(|addr| match addr {
                        mdns_sd::ScopedIp::V4(ip) => Some(ip.addr().to_string()),
                        _ => None,
                    })
                    .or_else(|| {
                        resolved.addresses.iter().find_map(|addr| match addr {
                            mdns_sd::ScopedIp::V6(ip)
                                if !ip.addr().is_unicast_link_local() =>
                            {
                                Some(format!("[{}]", ip.addr()))
                            }
                            _ => None,
                        })
                    });

                let Some(host) = host else {
                    log::info!(
                        "Resolved mdns service {mailbox_id} has no usable addresses (only link-local IPv6?), waiting for next announcement"
                    );
                    continue;
                };

                let url = format!("http://{host}:{port}");
                node.mailboxes
                    .register(mailbox_client::toy::ToyMailboxClient::new(
                        mailbox_id.clone(),
                        url.clone(),
                    ))
                    .await;
                log::info!(
                    "*** Registered local mailbox client via mdns: {mailbox_id} ({url}) ***",
                );
            }
            mdns_sd::ServiceEvent::ServiceRemoved(ty_domain, fullname) => {
                let mailbox_id = instance_name_from_fullname(&fullname, &ty_domain);
                if node.mailboxes.unregister(&mailbox_id).await {
                    log::info!(
                        "*** Removed local mailbox client via mdns: {mailbox_id} ***"
                    );
                }
            }
            other_event => {
                log::trace!("((( Received other mdns event: {:?} )))", &other_event);
            }
        }
    }

    log::debug!("mdns browse handler loop ended");
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

/// Drain interface events for 500ms so a burst of related changes (typical
/// during a Wi-Fi handoff) collapses into a single refresh.
async fn debounce_browse_burst(watcher: &mut if_watch::tokio::IfWatcher) {
    use futures::StreamExt;
    let debounce = tokio::time::sleep(std::time::Duration::from_millis(500));
    tokio::pin!(debounce);
    loop {
        tokio::select! {
            biased;
            _ = &mut debounce => return,
            next = watcher.next() => {
                if next.is_none() {
                    return;
                }
            }
        }
    }
}
