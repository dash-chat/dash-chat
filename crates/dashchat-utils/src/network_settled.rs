//! One debounced "the network changed and has settled" signal, shared by every
//! consumer in the process.
//!
//! `if_watch` surfaces interface up/down events, and a single transition emits a
//! burst of them — often mid-switch, before the new network is usable. So we
//! coalesce: wait until the interfaces go quiet, then emit once.
//!
//! Settled is not the same as reachable: a captive portal, a LAN-only network,
//! or an upstream router recovering with no local interface change all break the
//! equivalence. Treat a tick as a hint to try, never as a promise.

use std::sync::OnceLock;
use std::time::Duration;

use futures::{Stream, StreamExt};
use tokio::sync::broadcast;

const SETTLE: Duration = Duration::from_millis(1500);
const CAPACITY: usize = 4;

/// Subscribes to debounced network-settled events.
///
/// The first call spawns the single interface watcher; later calls hand out
/// further receivers of the same signal.
pub fn network_settled() -> broadcast::Receiver<()> {
    static SENDER: OnceLock<broadcast::Sender<()>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(CAPACITY);
            spawn_watcher(tx.clone());
            tx
        })
        .subscribe()
}

fn spawn_watcher(tx: broadcast::Sender<()>) {
    tokio::spawn(async move {
        let watcher = match if_watch::tokio::IfWatcher::new() {
            Ok(watcher) => watcher,
            Err(err) => {
                tracing::warn!(?err, "network-settled: failed to start interface watcher");
                return;
            }
        };
        let events = watcher.filter_map(|event| async move {
            match event {
                Ok(_) => Some(()),
                Err(err) => {
                    tracing::warn!(?err, "network-settled: interface watcher error");
                    None
                }
            }
        });
        settle(events, SETTLE, move || {
            tracing::info!("network-settled: network settled");
            // Err just means nobody is listening right now.
            let _ = tx.send(());
        })
        .await;
        tracing::warn!("network-settled: interface watcher stream ended");
    });
}

/// Calls `on_settled` once per burst: after each event, waits for `quiet_for`
/// of silence before firing. Returns when `stream` ends.
async fn settle<S>(stream: S, quiet_for: Duration, mut on_settled: impl FnMut())
where
    S: Stream<Item = ()>,
{
    let stream = stream.fuse();
    futures::pin_mut!(stream);
    while stream.next().await.is_some() {
        while tokio::time::timeout(quiet_for, stream.next())
            .await
            .is_ok_and(|event| event.is_some())
        {}
        on_settled();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use futures::stream;

    #[tokio::test(start_paused = true)]
    async fn a_burst_of_interface_events_settles_into_one_tick() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counter = ticks.clone();

        let events = stream::iter(vec![(), (), (), ()]);
        settle(events, Duration::from_millis(1500), move || {
            counter.fetch_add(1, Ordering::Relaxed);
        })
        .await;

        assert_eq!(ticks.load(Ordering::Relaxed), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn two_transitions_separated_by_quiet_are_two_ticks() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counter = ticks.clone();

        let events = stream::unfold(0usize, |sent| async move {
            match sent {
                0 | 1 => Some(((), sent + 1)),
                2 => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Some(((), 3))
                }
                _ => None,
            }
        });
        settle(events, Duration::from_millis(1500), move || {
            counter.fetch_add(1, Ordering::Relaxed);
        })
        .await;

        assert_eq!(ticks.load(Ordering::Relaxed), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn no_events_is_no_tick() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counter = ticks.clone();

        settle(
            stream::empty::<()>(),
            Duration::from_millis(1500),
            move || {
                counter.fetch_add(1, Ordering::Relaxed);
            },
        )
        .await;

        assert_eq!(ticks.load(Ordering::Relaxed), 0);
    }
}
