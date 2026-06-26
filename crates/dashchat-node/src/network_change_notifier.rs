//! Forwards OS network changes to iroh.
//!
//! iroh cannot detect network changes by itself on Android — its native network
//! monitor has no working backend there (see [`iroh::Endpoint::network_change`]).
//! Without this, after a WiFi/cellular switch iroh keeps stale addresses and
//! sockets and never reconnects until the process restarts. `if_watch` does
//! surface interface up/down events on Android; a single transition emits a
//! burst of them, often mid-switch before the new network is usable, and iroh
//! rebinds only once per notification (iroh#4289), so we debounce: wait until
//! the interfaces go quiet, then notify iroh once. Non-Android platforms detect
//! changes natively, so this is a no-op there.

#[allow(unused_imports)]
use tokio::task::JoinHandle;

/// Spawn the network-change notifier for the given endpoint.
#[cfg(target_os = "android")]
pub(crate) fn spawn(endpoint: p2panda::Endpoint) -> JoinHandle<()> {
    use futures::StreamExt;
    use std::time::Duration;

    // How long interfaces must stay quiet before we treat the transition as
    // settled and notify iroh.
    const SETTLE: Duration = Duration::from_millis(1500);

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

            match endpoint.endpoint().await {
                Ok(iroh) => {
                    tracing::info!("network-change notifier: network settled, notifying iroh");
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
    })
}

#[cfg(not(target_os = "android"))]
pub(crate) fn spawn(_endpoint: p2panda::Endpoint) -> JoinHandle<()> {
    // iroh detects network changes itself on non-Android platforms.
    tokio::spawn(async {})
}
