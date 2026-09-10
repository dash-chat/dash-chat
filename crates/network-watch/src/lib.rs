//! One debounced "the network changed and has settled" signal, shared by every
//! consumer in the process.
//!
//! A change is noticed two ways: `if_watch` surfaces interface up/down events,
//! and the interface addresses are polled — netlink hands Android apps the same
//! change seconds late, and a user reads anything slower than a couple of
//! seconds as broken. A single transition emits a burst of events either way,
//! often mid-switch, so we coalesce: wait until they go quiet, then emit once.
//!
//! Settled is not the same as reachable: a captive portal, a LAN-only network,
//! or an upstream router recovering with no local interface change all break the
//! equivalence. Treat a tick as a hint to try, never as a promise.

use std::collections::BTreeSet;
use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

use futures::{stream, Stream, StreamExt};
use tokio::sync::broadcast;

const POLL: Duration = Duration::from_millis(250);
const SETTLE: Duration = Duration::from_millis(300);
const CAPACITY: usize = 4;

/// A receiver that ticks once each time the network changes and settles.
///
/// The first call spawns the single interface watcher; later calls hand out
/// further receivers of the same signal.
pub fn network_change() -> broadcast::Receiver<()> {
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
                tracing::warn!(?err, "network-change: failed to start interface watcher");
                return;
            }
        };
        let events = watcher.filter_map(|event| async move {
            match event {
                Ok(if_watch::IfEvent::Up(net) | if_watch::IfEvent::Down(net)) => {
                    is_routable(net.addr()).then_some(())
                }
                Err(err) => {
                    tracing::warn!(?err, "network-change: interface watcher error");
                    None
                }
            }
        });
        let events = stream::select(events, address_changes());
        settle(events, SETTLE, move || {
            tracing::info!("network-change: network settled");
            // Err just means nobody is listening right now.
            let _ = tx.send(());
        })
        .await;
        tracing::warn!("network-change: interface watcher stream ended");
    });
}

/// An event whenever the set of routable interface addresses differs from the
/// last poll — netlink surfaces the same change to Android apps seconds late, so
/// polling catches it within `POLL` instead of waiting on the watcher.
fn address_changes() -> impl Stream<Item = ()> {
    stream::unfold(routable_addresses(), |last| async move {
        loop {
            tokio::time::sleep(POLL).await;
            let now = routable_addresses();
            if now != last {
                tracing::debug!(addresses = ?now, "network-change: addresses changed");
                return Some(((), now));
            }
        }
    })
}

/// The interfaces' addresses that can reach anything, as `(interface, address)`.
///
/// Loopback and link-local addresses are left out: Android hands an interface
/// its link-local address seconds before DHCP delivers the routable one, and
/// reacting to the former only wastes the reaction on a phone that cannot reach
/// anything yet.
fn routable_addresses() -> BTreeSet<(String, IpAddr)> {
    if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .map(|interface| (interface.name.clone(), interface.ip()))
                .filter(|(_, ip)| is_routable(*ip))
                .collect()
        })
        .unwrap_or_default()
}

fn is_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local(),
        IpAddr::V6(v6) => !v6.is_loopback() && !v6.is_unicast_link_local(),
    }
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
