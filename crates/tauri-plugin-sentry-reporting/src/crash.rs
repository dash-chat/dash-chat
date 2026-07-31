use std::path::{Path, PathBuf};
use std::sync::Weak;

use sentry::integrations::panic::PanicIntegration;
use sentry::Envelope;

use crate::envelope;
use crate::state::{Sentry, SentryState};

/// Sentry's own on-the-wire format, so what a run that died left behind needs no
/// shape of its own.
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

/// Keeps a panic for the next launch to offer; release builds abort, so this is
/// the only chance to keep it.
pub(crate) fn install_hook(state: Weak<SentryState>) {
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
    let path = pending_crash_path(data_dir);
    if path.exists() {
        return;
    }
    if let Err(err) = write_atomically(&path, envelope) {
        log::error!("sentry-reporting: could not keep the crash report: {err}");
    }
}

/// Written beside the target and renamed, so a launch never reads half a file.
fn write_atomically(path: &Path, envelope: &Envelope) -> std::io::Result<()> {
    let staged = path.with_extension("tmp");
    envelope.to_writer(std::fs::File::create(&staged)?)?;
    std::fs::rename(&staged, path)
}

fn has_pending_crash(data_dir: &Path) -> bool {
    pending_crash_path(data_dir).exists()
}

/// Reads and removes, so a crash is offered exactly once. Unreadable counts as
/// read: better dropped than offered at every launch.
fn take_pending_crash(data_dir: &Path) -> Option<Envelope> {
    let envelope = Envelope::from_path(pending_crash_path(data_dir)).ok();
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

    use sentry::protocol::{EnvelopeItem, Event, ItemContainer, Level, Log};

    use crate::testing::{log_saying, plugin};

    fn saying(message: &str) -> Envelope {
        Event {
            message: Some(message.into()),
            ..Default::default()
        }
        .into()
    }

    fn logs(envelope: &Envelope) -> Vec<Log> {
        envelope
            .items()
            .find_map(|item| match item {
                EnvelopeItem::ItemContainer(ItemContainer::Logs(logs)) => Some(logs.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Sets a process-wide hook; nextest runs each test in its own process.
    fn panicking_plugin(dir: &Path) -> Arc<SentryState> {
        let (state, _) = plugin(dir);
        install_hook(Arc::downgrade(&state));
        state
    }

    #[test]
    fn a_panic_leaves_a_report_for_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let _state = panicking_plugin(dir.path());

        let _ = std::panic::catch_unwind(|| panic!("boom in secret-abc123"));

        assert!(has_pending_crash(dir.path()));
        let stored = take_pending_crash(dir.path()).expect("no crash stored");
        let event = stored.event().expect("no event in the envelope");
        assert_eq!(event.level, Level::Fatal);
        // Prepared when stored, so the report is ready to send as-is.
        assert_eq!(
            event.exception[0].value.as_deref(),
            Some("boom in [REDACTED]")
        );
        assert!(!event.debug_meta.images.is_empty());
    }

    #[test]
    fn a_panic_keeps_the_logs_that_led_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = panicking_plugin(dir.path());
        state
            .client
            .capture_log(log_saying("before the fall"), &Default::default());

        let _ = std::panic::catch_unwind(|| panic!("boom"));

        let stored = logs(&take_pending_crash(dir.path()).expect("no crash stored"));
        assert!(
            stored.iter().any(|log| log.body == "before the fall"),
            "got: {stored:?}"
        );
    }

    #[test]
    fn stored_logs_are_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let state = panicking_plugin(dir.path());
        state
            .client
            .capture_log(log_saying("token secret-abc123"), &Default::default());

        let _ = std::panic::catch_unwind(|| panic!("boom"));

        let stored = logs(&take_pending_crash(dir.path()).expect("no crash stored"));
        assert_eq!(stored[0].body, "token [REDACTED]");
    }

    /// The logs ride the same file, so they have to survive the round trip too.
    #[test]
    fn a_stored_crash_keeps_the_trace_tying_its_logs_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = panicking_plugin(dir.path());
        state
            .client
            .capture_log(log_saying("before the fall"), &Default::default());

        let _ = std::panic::catch_unwind(|| panic!("boom"));

        let stored = take_pending_crash(dir.path()).expect("no crash stored");
        let Some(sentry::protocol::Context::Trace(trace)) =
            stored.event().unwrap().contexts.get("trace")
        else {
            panic!("no trace context on the event");
        };
        assert!(logs(&stored)
            .iter()
            .all(|log| log.trace_id == Some(trace.trace_id)));
    }

    #[test]
    fn taking_clears_it_so_it_is_offered_once() {
        let dir = tempfile::tempdir().unwrap();
        keep_for_next_launch(dir.path(), &saying("boom"));

        assert!(take_pending_crash(dir.path()).is_some());
        assert!(!has_pending_crash(dir.path()));
        assert!(take_pending_crash(dir.path()).is_none());
    }

    #[test]
    fn an_unreadable_report_is_dropped_rather_than_offered_forever() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(pending_crash_path(dir.path()), "not an envelope").unwrap();

        assert!(take_pending_crash(dir.path()).is_none());
        assert!(!has_pending_crash(dir.path()));
    }
}
