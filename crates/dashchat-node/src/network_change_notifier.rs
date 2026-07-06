//! Wakes the mailbox poller when the OS network configuration changes.
//!
//! The mailbox manager backs off to slow polling (up to `stopped_interval`)
//! while offline, so without a nudge a restored connection waits out the full
//! backoff before the next sync. `wakeup_all()` resets every mailbox to an
//! immediate poll, making reconnection feel instant.
//!
//! `if_watch` surfaces interface up/down events; a single transition emits a
//! burst of them, often mid-switch before the new network is usable, so we
//! debounce: wait until the interfaces go quiet, then wake the mailboxes once.

use futures::StreamExt;
use std::time::Duration;
use tokio::task::JoinHandle;

use mailbox_client::manager::Mailboxes;

use crate::mailbox::MailboxOperation;
use crate::stores::OpStore;

// How long interfaces must stay quiet before we treat the transition as
// settled.
const SETTLE: Duration = Duration::from_millis(1500);

/// Spawn the network-change notifier for the given mailboxes.
pub(crate) fn spawn(mailboxes: Mailboxes<MailboxOperation, OpStore>) -> JoinHandle<()> {
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
        }
    })
}
