use std::path::{Path, PathBuf};
use std::sync::Weak;

use sentry::integrations::panic::PanicIntegration;
use sentry::Envelope;

use crate::envelope;
use crate::state::{Sentry, SentryState};

const FILE_NAME: &str = "pending-crash.envelope";

fn pending_crash_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

#[tauri::command]
pub(crate) async fn pending_crash_report(state: Sentry<'_>) -> Result<bool, String> {
    Ok(has_pending_crash(&state.data_dir))
}

#[tauri::command]
pub(crate) async fn send_pending_crash_report(state: Sentry<'_>) -> Result<(), String> {
    if let Some(envelope) = take_pending_crash(&state.data_dir) {
        envelope::send(&state, envelope).await;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn discard_pending_crash_report(state: Sentry<'_>) -> Result<(), String> {
    remove_pending_crash(&state.data_dir);
    Ok(())
}

/// Keeps a panic for the next launch to offer
pub(crate) fn install_panic_hook(state: Weak<SentryState>) {
    let next = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(state) = state.upgrade() {
            let event = PanicIntegration::new().event_from_panic_info(info);
            let logs = state.pending.snapshot();
            if let Some(envelope) = envelope::build_envelope(&state, event, logs) {
                keep_for_next_launch(&state.data_dir, &envelope);
            }
        }
        next(info);
    }));
}

fn keep_for_next_launch(data_dir: &Path, envelope: &Envelope) {
    if has_pending_crash(data_dir) {
        return;
    }
    if let Err(err) = std::fs::File::create(pending_crash_path(data_dir))
        .and_then(|file| envelope.to_writer(file))
    {
        log::error!("sentry-reporting: could not keep the crash report: {err}");
    }
}

fn read_pending_crash(data_dir: &Path) -> Option<Envelope> {
    Envelope::from_path(pending_crash_path(data_dir)).ok()
}

/// What cannot be read is no report: a death mid-write leaves half a file, which
/// must neither be offered nor stand in the way of the next crash.
fn has_pending_crash(data_dir: &Path) -> bool {
    read_pending_crash(data_dir).is_some()
}

/// Reads and removes, so a crash is offered exactly once
fn take_pending_crash(data_dir: &Path) -> Option<Envelope> {
    let envelope = read_pending_crash(data_dir);
    remove_pending_crash(data_dir);
    envelope
}

fn remove_pending_crash(data_dir: &Path) {
    let _ = std::fs::remove_file(pending_crash_path(data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use sentry::protocol::{Context, EnvelopeItem, Event, ItemContainer, Level, Log};

    use crate::testing::{log_saying, plugin};

    fn logs(envelope: &Envelope) -> Vec<Log> {
        envelope
            .items()
            .find_map(|item| match item {
                EnvelopeItem::ItemContainer(ItemContainer::Logs(logs)) => Some(logs.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_panic_leaves_a_report_the_next_launch_is_offered_once() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = plugin(dir.path());
        install_panic_hook(Arc::downgrade(&state));
        state
            .client
            .capture_log(log_saying("before the fall"), &Default::default());

        let _ = std::panic::catch_unwind(|| panic!("boom in secret-abc123"));

        let stored = take_pending_crash(dir.path()).expect("no crash stored");
        let event = stored.event().expect("no event in the envelope");
        assert_eq!(event.level, Level::Fatal);
        // Prepared when stored, so the report is ready to send as-is.
        assert_eq!(
            event.exception[0].value.as_deref(),
            Some("boom in [REDACTED]")
        );
        assert!(!event.debug_meta.images.is_empty());

        // Without a shared trace Sentry files the logs on their own rather than
        // against the issue.
        let Some(Context::Trace(trace)) = event.contexts.get("trace") else {
            panic!("no trace context on the event");
        };
        let stored_logs = logs(&stored);
        assert!(
            stored_logs.iter().any(|log| log.body == "before the fall"),
            "got: {stored_logs:?}"
        );
        assert!(stored_logs
            .iter()
            .all(|log| log.trace_id == Some(trace.trace_id)));

        assert!(!has_pending_crash(dir.path()));
    }

    #[test]
    fn an_unreadable_report_is_no_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(pending_crash_path(dir.path()), "half an envelope").unwrap();

        assert!(!has_pending_crash(dir.path()));

        let next: Envelope = Event {
            message: Some("boom".into()),
            ..Default::default()
        }
        .into();
        keep_for_next_launch(dir.path(), &next);

        let stored = take_pending_crash(dir.path()).expect("no crash stored");
        assert_eq!(
            stored
                .event()
                .expect("no event in the envelope")
                .message
                .as_deref(),
            Some("boom")
        );
    }
}
