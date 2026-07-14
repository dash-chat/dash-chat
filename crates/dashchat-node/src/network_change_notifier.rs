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
//! `if_watch` surfaces interface up/down events; a single transition emits a
//! burst of them, often mid-switch before the new network is usable, and iroh
//! rebinds only once per notification (iroh#4289), so we debounce: wait until
//! the interfaces go quiet, then react once.

use futures::StreamExt;
use std::time::Duration;
use tokio::task::JoinHandle;

use mailbox_client::manager::Mailboxes;

use crate::mailbox::MailboxOperation;
use crate::stores::OpStore;

// How long interfaces must stay quiet before we treat the transition as
// settled.
const SETTLE: Duration = Duration::from_millis(1500);

/// Spawn the network-change notifier for the given endpoint and mailboxes.
pub(crate) fn spawn(
    endpoint: p2panda::Endpoint,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut watcher = match if_watch::tokio::IfWatcher::new() {
            Ok(watcher) => watcher,
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "network-change notifier: failed to start interface watcher"
                );
                return;
            }
        };

        loop {
            // Block until the first interface event of a transition.
            match watcher.next().await {
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    tracing::warn!(?err, "network-change notifier: interface watcher error");
                    continue;
                }
                None => {
                    tracing::warn!("network-change notifier: interface watcher stream ended");
                    return;
                }
            }

            // Coalesce the rest of the burst until interfaces stay quiet.
            loop {
                match tokio::time::timeout(SETTLE, watcher.next()).await {
                    Ok(Some(_)) => continue,
                    Ok(None) => {
                        tracing::warn!("network-change notifier: interface watcher stream ended");
                        return;
                    }
                    Err(_) => break,
                }
            }

            tracing::info!("network-change notifier: network settled, waking mailbox sync");
            mailboxes.wakeup_all().await;
            notify_iroh(&endpoint).await;
        }
    })
}

#[cfg(target_os = "android")]
async fn notify_iroh(endpoint: &p2panda::Endpoint) {
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
