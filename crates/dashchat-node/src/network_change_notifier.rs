//! Reacts to OS network changes: wakes the mailbox poller everywhere, and
//! forwards the change to iroh on Android.
//!
//! The mailbox manager backs off to slow polling (up to `stopped_interval`)
//! while offline, so without a nudge a restored connection waits out the full
//! backoff before the next sync. `wakeup_all()` resets every mailbox to an
//! immediate poll, making reconnection feel instant on all platforms.
//!
//! iroh cannot detect network changes by itself on Android — its native network
//! monitor has no working backend there (see [`iroh::Endpoint::network_change`]).
//! Without this, after a WiFi/cellular switch iroh keeps stale addresses and
//! sockets and never reconnects until the process restarts. Non-Android
//! platforms detect changes natively, so iroh is only notified on Android.
//!
//! Detection and debouncing live in [`dashchat_utils::network_settled`], shared
//! with the other subsystems that need to know a connection came back.

use tokio::task::JoinHandle;

use mailbox_client::manager::Mailboxes;

use crate::mailbox::MailboxOperation;
use crate::stores::OpStore;

/// Spawn the network-change notifier for the given endpoint and mailboxes.
pub(crate) fn spawn(
    endpoint: p2panda::Endpoint,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut settled = dashchat_utils::network_settled();
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
            tracing::info!("network-change notifier: waking mailbox sync");
            mailboxes.wakeup_all().await;
            notify_iroh(&endpoint).await;
        }
    })
}

#[cfg(target_os = "android")]
async fn notify_iroh(endpoint: &p2panda::Endpoint) {
    use std::time::Duration;

    match endpoint.endpoint().await {
        Ok(iroh) => {
            tracing::info!("network-change notifier: notifying iroh");
            iroh.network_change().await;
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

#[cfg(not(target_os = "android"))]
async fn notify_iroh(_endpoint: &p2panda::Endpoint) {}
