//! Reacts to OS network changes: wakes the mailbox poller everywhere, and
//! forwards the change to iroh on Android.
//!
//! The mailbox manager backs off to slow polling (up to `stopped_interval`)
//! while offline, so without a nudge a restored connection waits out the full
//! backoff before the next sync. `probe_all()` polls every mailbox now,
//! making reconnection feel instant on all platforms. It is a probe rather
//! than a wakeup because an interface change says nothing about which
//! mailboxes are reachable: a hub left behind on the old network stays Stopped
//! unless the poll proves otherwise.
//!
//! iroh cannot detect network changes by itself on Android — its native network
//! monitor has no working backend there (see [`iroh::Endpoint::network_change`]).
//! Without this, after a WiFi/cellular switch iroh keeps stale addresses and
//! sockets and never reconnects until the process restarts. Non-Android
//! platforms detect changes natively, so iroh is only notified on Android.
//!
//! Even once notified, iroh recomputes the addresses it advertises only when a
//! net report's result differs from the previous one, which an offline report
//! never does, so a new LAN address reached peers only with iroh's periodic
//! re-scan 20-26 s later. Registering the interface addresses as external
//! addresses makes it recompute and republish at once.
//!
//! Detection and debouncing live in [`network_watch::network_change`], shared
//! with the other subsystems that need to know a connection came back.

use tokio::task::JoinHandle;

use mailbox_client::manager::Mailboxes;

use crate::mailbox::MailboxOperation;
use crate::stores::OpStore;

/// Spawn the network-change notifier for the given endpoint and mailboxes.
/// An offline node has no endpoint; it still wants its mailboxes probed.
pub(crate) fn spawn(
    endpoint: Option<p2panda::Endpoint>,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut settled = network_watch::network_change();
        loop {
            match settled.recv().await {
                Ok(()) => {}
                // Lagged means we missed a change; reacting once is still right.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::warn!("network-change notifier: signal closed");
                    return;
                }
            }
            tracing::info!("network-change notifier: probing mailboxes");
            mailboxes.probe_all().await;
            if let Some(endpoint) = &endpoint {
                notify_iroh(endpoint).await;
            }
        }
    })
}

#[cfg(target_os = "android")]
use std::collections::BTreeSet;
#[cfg(target_os = "android")]
use std::net::SocketAddr;

#[cfg(target_os = "android")]
async fn notify_iroh(endpoint: &p2panda::Endpoint) {
    use std::time::Duration;

    match endpoint.endpoint().await {
        Ok(iroh) => {
            tracing::info!("network-change notifier: notifying iroh");
            iroh.network_change().await;
            publish_local_addrs(&iroh).await;
            // Brief, bounded wait for iroh to re-establish; the offline-LAN
            // case never goes "online", so don't block the loop on it.
            let _ = tokio::time::timeout(Duration::from_secs(5), iroh.online()).await;
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                "network-change notifier: could not access iroh endpoint"
            );
        }
    }
}

/// Registers the current interface addresses with iroh as external addresses
/// and retracts the ones it still advertises from the previous network.
#[cfg(target_os = "android")]
async fn publish_local_addrs(iroh: &iroh::Endpoint) {
    let current = local_socket_addrs(iroh);
    for addr in iroh.addr().ip_addrs() {
        if !current.contains(addr) {
            iroh.remove_external_addr(addr).await;
        }
    }
    for addr in &current {
        iroh.add_external_addr(*addr).await;
    }
    tracing::info!(addrs = ?current, "network-change notifier: published local addresses");
}

/// Every routable interface address paired with the port iroh bound for that
/// address family.
#[cfg(target_os = "android")]
fn local_socket_addrs(iroh: &iroh::Endpoint) -> BTreeSet<SocketAddr> {
    let ips = network_watch::routable_ips();
    let mut addrs = BTreeSet::new();
    for bound in iroh.bound_sockets() {
        for ip in ips.iter().filter(|ip| ip.is_ipv4() == bound.is_ipv4()) {
            addrs.insert(SocketAddr::new(*ip, bound.port()));
        }
    }
    addrs
}

#[cfg(not(target_os = "android"))]
async fn notify_iroh(_endpoint: &p2panda::Endpoint) {}
