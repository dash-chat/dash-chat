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
    let mdns = handle.state::<ServiceDaemon>();
    let receiver = mdns.browse(MDNS_SERVICE_TYPE)?;
    log::info!("Started mdns browse for local mailboxes: {MDNS_SERVICE_TYPE}");

    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
                    let mailbox_id = resolved.fullname;
                    let port = resolved.port;

                    // Prefer IPv4, fall back to IPv6 with bracket notation
                    let host = resolved
                        .addresses
                        .iter()
                        .find_map(|addr| match addr {
                            mdns_sd::ScopedIp::V4(ip) => Some(ip.addr().to_string()),
                            _ => None,
                        })
                        .or_else(|| {
                            resolved.addresses.iter().find_map(|addr| match addr {
                                mdns_sd::ScopedIp::V6(ip) => Some(format!("[{}]", ip.addr())),
                                _ => None,
                            })
                        });

                    let Some(host) = host else {
                        log::warn!("Resolved mdns service {mailbox_id} has no addresses, skipping");
                        continue;
                    };

                    let n = node.clone();
                    n.mailboxes
                        .register(mailbox_client::toy::ToyMailboxClient::new(
                            mailbox_id.clone(),
                            format!("http://{host}:{port}"),
                        ))
                        .await;
                    log::info!(
                        "*** Added new local mailbox client via mdns: {mailbox_id} ({host}:{port}) ***",
                    );
                }
                other_event => {
                    log::trace!("((( Received other mdns event: {:?} )))", &other_event);
                }
            }
        }

        log::warn!("mdns discovery loop ended");
    });

    Ok(())
}
