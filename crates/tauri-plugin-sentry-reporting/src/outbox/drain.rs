//! Emptying the outbox: one pass at a time, woken by anything that suggests a
//! connection might exist.

use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use sentry::Envelope;
use tokio::sync::Mutex;

use crate::outbox::entry;
use crate::outbox::retention;
use crate::outbox::sender::{Delivery, EnvelopeSender};
use crate::outbox::Outbox;

const INITIAL_BACKOFF: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrainResult {
    /// Nothing is waiting any more.
    Emptied,
    /// Something is still waiting for a connection.
    Pending,
}

/// One pass over `queued/`, oldest first. Never touches `held/`.
pub(crate) async fn drain_once(outbox: &Outbox, sender: &impl EnvelopeSender) -> DrainResult {
    retention::enforce(outbox.root());

    let mut result = DrainResult::Emptied;
    for queued in outbox.queued() {
        // Renamed out of the way first, so no other drainer — in this process
        // or another sharing the data directory — picks up the same entry.
        let Ok(in_flight) = entry::mark_sending(&queued.path) else {
            continue;
        };
        let Some(envelope) = entry::read(&in_flight) else {
            continue;
        };

        match sender.post(&envelope).await {
            Delivery::Delivered | Delivery::Rejected => {
                let _ = std::fs::remove_file(&in_flight);
            }
            Delivery::Retry { .. } => {
                let _ = std::fs::rename(&in_flight, &queued.path);
                result = DrainResult::Pending;
                // A failure now means the rest will fail too; stop wasting the
                // attempt and let the backoff decide when to try again.
                break;
            }
        }
    }
    result
}

/// Owns the background drain loop and serializes every trigger through it.
pub(crate) struct Drainer<S: EnvelopeSender> {
    outbox: Arc<Outbox>,
    sender: Arc<S>,
    draining: Arc<Mutex<()>>,
}

impl<S: EnvelopeSender + 'static> Drainer<S> {
    /// Starts draining on startup, on every network-settled tick, and on a
    /// backoff timer while anything is still waiting.
    pub(crate) fn spawn(outbox: Arc<Outbox>, sender: Arc<S>) -> Arc<Self> {
        let drainer = Arc::new(Self {
            outbox,
            sender,
            draining: Arc::new(Mutex::new(())),
        });

        let background = drainer.clone();
        tauri::async_runtime::spawn(async move {
            let mut settled = dashchat_utils::network_settled();
            let mut backoff = INITIAL_BACKOFF;
            loop {
                match background.drain_now().await {
                    DrainResult::Emptied => backoff = INITIAL_BACKOFF,
                    DrainResult::Pending => {
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                    }
                }
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    changed = settled.recv() => {
                        if matches!(changed, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            return;
                        }
                        backoff = INITIAL_BACKOFF;
                    }
                }
            }
        });

        drainer
    }

    /// Waits for any drain already running, so a report is never posted twice.
    pub(crate) async fn drain_now(&self) -> DrainResult {
        let _guard = self.draining.lock().await;
        drain_once(&self.outbox, self.sender.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use sentry::protocol::Event;

    #[derive(Default)]
    struct FakeSender {
        deliveries: Mutex<Vec<Delivery>>,
        posted: AtomicUsize,
    }

    impl FakeSender {
        fn always(delivery: Delivery) -> Self {
            Self {
                deliveries: Mutex::new(vec![delivery]),
                posted: AtomicUsize::new(0),
            }
        }

        fn posted(&self) -> usize {
            self.posted.load(Ordering::Relaxed)
        }
    }

    impl EnvelopeSender for FakeSender {
        async fn post(&self, _envelope: &Envelope) -> Delivery {
            self.posted.fetch_add(1, Ordering::Relaxed);
            let mut deliveries = self.deliveries.lock().unwrap();
            if deliveries.len() > 1 {
                deliveries.remove(0)
            } else {
                deliveries[0].clone()
            }
        }
    }

    fn envelope(message: &str) -> Envelope {
        Event {
            message: Some(message.into()),
            ..Default::default()
        }
        .into()
    }

    #[tokio::test]
    async fn a_delivered_report_leaves_the_outbox() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        let result = drain_once(&outbox, &sender).await;

        assert_eq!(result, DrainResult::Emptied);
        assert_eq!(sender.posted(), 1);
        assert!(outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn an_undeliverable_report_stays_for_the_next_try() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Retry { after: None });

        let result = drain_once(&outbox, &sender).await;

        assert_eq!(result, DrainResult::Pending);
        assert_eq!(outbox.queued().len(), 1);
    }

    #[tokio::test]
    async fn a_rejected_report_is_dropped_rather_than_retried_forever() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Rejected);

        let result = drain_once(&outbox, &sender).await;

        assert_eq!(result, DrainResult::Emptied);
        assert!(outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn a_held_crash_is_never_drained() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.hold(&envelope("crash")).unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        drain_once(&outbox, &sender).await;

        assert_eq!(sender.posted(), 0);
        assert!(outbox.has_held());
    }

    #[tokio::test]
    async fn every_queued_report_is_attempted_in_one_drain() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("one")).unwrap();
        outbox.enqueue(&envelope("two")).unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        drain_once(&outbox, &sender).await;

        assert_eq!(sender.posted(), 2);
        assert!(outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn a_report_left_in_flight_is_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Retry { after: None });

        drain_once(&outbox, &sender).await;

        // Restored to queued/ rather than left as .sending.
        let sending: Vec<_> = walkdir_sending_files(outbox.root());
        assert!(sending.is_empty(), "a .sending file survived: {sending:?}");
    }

    fn walkdir_sending_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        fn visit(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "sending") {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        visit(root, &mut out);
        out
    }
}
