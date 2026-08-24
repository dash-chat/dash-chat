//! Emptying the outbox: one pass at a time, woken by anything that suggests a
//! connection might exist.

use std::path::{Path, PathBuf};
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
/// How long to wait before rescanning an outbox that had nothing in it.
const IDLE_INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrainResult {
    /// Nothing is waiting any more.
    Emptied,
    /// Something is still waiting for a connection.
    Pending,
}

/// What became of one particular entry, as opposed to the pass as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryFate {
    /// Sentry has it.
    Delivered,
    /// Still on disk, waiting for a connection.
    Waiting,
    /// Gone without being delivered: refused by Sentry, or evicted by retention.
    Dropped,
}

/// The outcome of one pass: its verdict, plus what it delivered.
pub(crate) struct DrainReport {
    pub(crate) result: DrainResult,
    delivered: Vec<PathBuf>,
}

impl DrainReport {
    /// What became of the entry originally written at `path`.
    pub(crate) fn fate(&self, path: &Path) -> EntryFate {
        if self.delivered.iter().any(|done| done == path) {
            EntryFate::Delivered
        } else if path.exists() {
            EntryFate::Waiting
        } else {
            EntryFate::Dropped
        }
    }
}

/// One pass over `queued/`, oldest first. Never touches `held/`.
pub(crate) async fn drain_once(outbox: &Outbox, sender: &impl EnvelopeSender) -> DrainReport {
    retention::enforce(outbox.root());

    let mut result = DrainResult::Emptied;
    let mut delivered = Vec::new();
    for queued in outbox.queued() {
        // Renamed out of the way first, so no other drainer — in this process
        // or another sharing the data directory — picks up the same entry.
        // A skipped entry was never delivered, so the pass cannot claim the
        // outbox is empty.
        let Ok(in_flight) = entry::mark_sending(&queued.path) else {
            result = DrainResult::Pending;
            continue;
        };
        let Some(envelope) = entry::read(&in_flight) else {
            result = DrainResult::Pending;
            continue;
        };

        match sender.post(&envelope).await {
            Delivery::Delivered => {
                let _ = std::fs::remove_file(&in_flight);
                delivered.push(queued.path.clone());
            }
            Delivery::Rejected => {
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
    DrainReport { result, delivered }
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
                let delay = match background.drain_now().await {
                    DrainResult::Emptied => {
                        backoff = INITIAL_BACKOFF;
                        IDLE_INTERVAL
                    }
                    DrainResult::Pending => {
                        let delay = backoff;
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                        delay
                    }
                };
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    changed = settled.recv() => {
                        if matches!(changed, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            log::warn!(
                                "sentry-reporting: the network-settled channel closed; \
                                 reports will only go out on the next launch"
                            );
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
        drain_once(&self.outbox, self.sender.as_ref()).await.result
    }

    /// Drains, and reports what became of `watched` rather than of the pass.
    pub(crate) async fn drain_watching(&self, watched: &Path) -> EntryFate {
        let _guard = self.draining.lock().await;
        drain_once(&self.outbox, self.sender.as_ref())
            .await
            .fate(watched)
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

        let queued = outbox.queued()[0].path.clone();

        let report = drain_once(&outbox, &sender).await;

        assert_eq!(report.result, DrainResult::Emptied);
        assert_eq!(report.fate(&queued), EntryFate::Delivered);
        assert_eq!(sender.posted(), 1);
        assert!(outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn an_undeliverable_report_stays_for_the_next_try() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Retry { after: None });
        let queued = outbox.queued()[0].path.clone();

        let report = drain_once(&outbox, &sender).await;

        assert_eq!(report.result, DrainResult::Pending);
        assert_eq!(report.fate(&queued), EntryFate::Waiting);
        assert_eq!(outbox.queued().len(), 1);
    }

    #[tokio::test]
    async fn a_rejected_report_is_dropped_rather_than_retried_forever() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        let queued = outbox.enqueue(&envelope("feedback")).unwrap();
        let sender = FakeSender::always(Delivery::Rejected);

        let report = drain_once(&outbox, &sender).await;

        // The pass has nothing left to do, but this report was never filed.
        assert_eq!(report.result, DrainResult::Emptied);
        assert_eq!(report.fate(&queued), EntryFate::Dropped);
        assert!(outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn a_report_evicted_by_retention_is_never_reported_as_sent() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        let oversized = "x".repeat(retention::MAX_BYTES as usize + 1);
        let queued = outbox.enqueue(&envelope(&oversized)).unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        let report = drain_once(&outbox, &sender).await;

        assert_eq!(sender.posted(), 0);
        assert_eq!(report.fate(&queued), EntryFate::Dropped);
    }

    #[tokio::test]
    async fn a_delivered_report_is_not_dragged_down_by_a_corrupt_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        let corrupt = outbox.enqueue(&envelope("first")).unwrap();
        std::fs::write(&corrupt, "half an envelope").unwrap();
        let mine = outbox.enqueue(&envelope("second")).unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        let report = drain_once(&outbox, &sender).await;

        assert_eq!(report.result, DrainResult::Pending);
        assert_eq!(report.fate(&mine), EntryFate::Delivered);
    }

    #[tokio::test]
    async fn an_unreadable_entry_is_never_reported_as_sent() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.enqueue(&envelope("feedback")).unwrap();
        std::fs::write(&outbox.queued()[0].path, "half an envelope").unwrap();
        let sender = FakeSender::always(Delivery::Delivered);

        let report = drain_once(&outbox, &sender).await;

        assert_eq!(report.result, DrainResult::Pending);
        assert_eq!(sender.posted(), 0);
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
