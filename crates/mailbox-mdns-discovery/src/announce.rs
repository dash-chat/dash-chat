//! Announce a mailbox as a DNS-SD service and keep the announcement alive
//! across network-interface changes.

use std::sync::{Arc, Mutex as StdMutex};

use mdns_sd::{ServiceDaemon, ServiceInfo};

/// Register the mailbox as an mDNS service, retrying up to `attempts` times.
/// `instance_name` is the mailbox's MailboxId. Returns the registered service
/// fullname (needed later to unregister).
pub fn register_mdns_with_retry(
    daemon: &ServiceDaemon,
    service_type: &str,
    instance_name: &str,
    port: u16,
    attempts: u32,
) -> anyhow::Result<String> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        let service = mdns_service_info(service_type, instance_name, port)?;
        let fullname = service.get_fullname().to_string();
        log::info!(
            "Registering local mailbox service via mdns: {} ({})",
            fullname,
            service.get_type()
        );
        match daemon.register(service) {
            Ok(()) => return Ok(fullname),
            Err(e) => {
                log::error!(
                    "Failed to register local mailbox service via mdns, attempt {attempt} of {attempts}, error: {e:?}"
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow::anyhow!("failed to register local mailbox service via mdns")))
}

/// Re-announce the mDNS record whenever a network interface comes up or down;
/// runs until cancelled. `enable_addr_auto()` only enumerates interfaces at
/// registration time, so without this the announcement misses interfaces (like
/// macOS Internet Sharing's bridge100) that appear after the mailbox starts.
///
/// `fullname` is shared with the caller so it always reflects the currently
/// registered service: the loop overwrites it on every re-announce so a later
/// unregister targets the actually-registered service rather than a stale one.
pub async fn reannounce_on_interface_change_loop(
    daemon: ServiceDaemon,
    service_type: String,
    instance_name: String,
    port: u16,
    fullname: Arc<StdMutex<String>>,
) {
    let mut watcher = match if_watch::tokio::IfWatcher::new() {
        Ok(w) => w,
        Err(err) => {
            log::warn!("Failed to start mailbox interface watcher: {err:?}");
            return;
        }
    };

    while dashchat_utils::wait_for_interface_change(&mut watcher).await {
        let prev = fullname.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(new_fullname) =
            reannounce_mdns(&daemon, &service_type, &prev, &instance_name, port)
        {
            *fullname.lock().unwrap_or_else(|p| p.into_inner()) = new_fullname;
        }
    }
}

/// Returns the fullname that is now registered with the daemon, or `None` if
/// the re-register failed (in which case the previous `fullname` is no longer
/// registered either — the unregister already ran).
fn reannounce_mdns(
    daemon: &ServiceDaemon,
    service_type: &str,
    fullname: &str,
    instance_name: &str,
    port: u16,
) -> Option<String> {
    let _ = daemon.unregister(fullname);
    let service = match mdns_service_info(service_type, instance_name, port) {
        Ok(service) => service,
        Err(err) => {
            log::warn!("Failed to build mDNS service info for re-announce: {err:?}");
            return None;
        }
    };
    let new_fullname = service.get_fullname().to_string();
    if let Err(err) = daemon.register(service) {
        log::warn!("Failed to re-register mailbox mDNS: {err:?}");
        return None;
    }
    Some(new_fullname)
}

fn mdns_service_info(
    service_type: &str,
    instance_name: &str,
    port: u16,
) -> anyhow::Result<ServiceInfo> {
    // The instance name is the mailbox's MailboxId (base64url-no-pad encoding
    // of the endpoint's 32-byte public key — 43 chars, fits a single DNS
    // label), so the mDNS instance name IS the canonical MailboxId.
    //
    // Per-device hostname so the A/AAAA owner-name doesn't collide with every
    // other Dash Chat instance on the LAN. A shared hostname can cause one
    // instance's address cache entry to overwrite another's in the resolver.
    let host_name = format!("{instance_name}.local.");

    Ok(
        ServiceInfo::new(service_type, instance_name, &host_name, "", port, vec![])?
            .enable_addr_auto(),
    )
}
