use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use sentry::integrations::log::log_from_record;
use sentry::protocol::{EnvelopeItem, ItemContainer, Log, TraceId};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::fern::{Dispatch, Output};
use tauri_plugin_log::{Target, TargetKind};

use crate::state::SentryState;

/// The attachment holds the whole history, so this only has to span the window
/// worth indexing
const MAX_ENTRIES: usize = 500;

#[derive(Default)]
pub(crate) struct Pending(Mutex<VecDeque<Log>>);

impl Pending {
    pub(crate) fn push(&self, log: Log) {
        let mut buf = self.lock();
        buf.push_back(log);
        while buf.len() > MAX_ENTRIES {
            buf.pop_front();
        }
    }

    /// A copy rather than a drain, so a second report still gets the context of
    /// the first.
    pub(crate) fn snapshot(&self) -> Vec<Log> {
        self.lock().iter().cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<Log>> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }
}

pub(crate) fn envelope_item(mut logs: Vec<Log>, trace_id: TraceId) -> EnvelopeItem {
    for log in &mut logs {
        log.trace_id = Some(trace_id);
    }
    EnvelopeItem::ItemContainer(ItemContainer::Logs(logs))
}

/// Keeps records for a report to carry
/// Add it to the tauri-plugin-log's `.targets([..])`
pub fn log_target<R: Runtime>(app: &AppHandle<R>) -> Target {
    let app = app.clone();
    Target::new(TargetKind::Dispatch(Dispatch::new().chain(Output::call(
        move |record| {
            if let Some(state) = app.try_state::<Arc<SentryState>>() {
                state
                    .client
                    .capture_log(log_from_record(record), &Default::default());
            }
        },
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::log_saying;
    #[test]
    fn keeps_the_newest_entries_in_order() {
        let pending = Pending::default();
        for i in 0..MAX_ENTRIES + 10 {
            pending.push(log_saying(&i.to_string()));
        }

        let logs = pending.snapshot();
        assert_eq!(logs.len(), MAX_ENTRIES);
        assert_eq!(logs[0].body, "10");
        assert_eq!(logs[MAX_ENTRIES - 1].body, (MAX_ENTRIES + 9).to_string());
    }

    #[test]
    fn snapshotting_leaves_the_entries_in_place() {
        let pending = Pending::default();
        pending.push(log_saying("kept"));

        assert_eq!(pending.snapshot().len(), 1);
        assert_eq!(pending.snapshot().len(), 1);
    }
}
