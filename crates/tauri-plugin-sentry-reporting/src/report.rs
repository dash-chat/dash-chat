use std::path::Path;

use sentry::protocol::{Attachment, EnvelopeItem, Event, Exception, Level, Log, TraceContext};
use sentry::types::protocol::latest::TraceId;
use sentry::Envelope;
use serde::Deserialize;

use crate::state::{Sentry, SentryState};
use crate::{logs, redaction};

const MAX_ATTACHED_LOG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct ReportedError {
    pub name: String,
    pub message: String,
    pub stack: Option<String>,
}

#[tauri::command]
pub(crate) async fn send_error_report(
    state: Sentry<'_>,
    message: String,
    error: Option<ReportedError>,
) -> Result<(), String> {
    let mut event = Event {
        message: Some(message),
        level: Level::Error,
        ..Default::default()
    };
    if let Some(error) = error {
        if let Some(stack) = error.stack {
            event.extra.insert("stack".into(), stack.into());
        }
        // Sentry titles and groups by the exception.
        event.exception = vec![Exception {
            ty: error.name,
            value: Some(error.message),
            ..Default::default()
        }]
        .into();
    }

    let logs = state.pending.snapshot();
    let state = state.inner().clone();
    // Reading a megabyte of log and running the patterns over it — and over the
    // event itself — is blocking and CPU-bound, so it stays off the async worker.
    tauri::async_runtime::spawn_blocking(move || send(&state, event, logs))
        .await
        .map_err(|err| format!("Report task failed: {err}"))
}

/// The only path to the network. Logs and the log file overlap on purpose: the
/// logs are searchable but recent, the file reaches further back.
fn send(state: &SentryState, mut event: Event<'static>, logs: Vec<Log>) {
    // Shared with every log below, which is what ties them to this issue.
    let trace_id = TraceId::default();
    event.contexts.insert(
        "trace".into(),
        TraceContext {
            trace_id,
            ..Default::default()
        }
        .into(),
    );

    // Redacts in `before_send`, exactly as `capture_event` would have.
    let Some(event) = state.client.prepare_event(event, None) else {
        return;
    };

    let mut envelope: Envelope = event.into();
    if !logs.is_empty() {
        envelope.add_item(logs::envelope_item(logs, trace_id));
    }
    if let Some(attachment) = read_log_attachment(state) {
        envelope.add_item(EnvelopeItem::Attachment(attachment));
    }
    state.gate.send(envelope);
}

fn read_log_attachment(state: &SentryState) -> Option<Attachment> {
    match redaction::redacted_log_tail(&state.redact, &state.logs_dir, MAX_ATTACHED_LOG_BYTES) {
        Ok(text) => Some(Attachment {
            buffer: text.into_bytes(),
            filename: log_file_name(&state.logs_dir),
            content_type: Some("text/plain".into()),
            ty: None,
        }),
        Err(err) => {
            log::warn!("sentry-reporting: could not attach the log: {err}");
            None
        }
    }
}

/// Names the attachment after the log it was tailed from, e.g. `Dash Chat.log`.
fn log_file_name(logs_dir: &Path) -> String {
    redaction::list_log_files_oldest_first(logs_dir)
        .ok()
        .and_then(|files| Some(files.last()?.file_name()?.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app.log".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::ItemContainer;

    use crate::testing::{log_saying, plugin};

    #[test]
    fn a_report_carries_the_event_the_logs_and_the_log_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.log"), "a line\n").unwrap();
        let (state, recorder) = plugin(dir.path().to_path_buf());

        send(&state, Event::default(), vec![log_saying("connecting")]);

        let envelope = recorder.only();
        assert!(envelope.event().is_some());
        assert!(envelope
            .items()
            .any(|item| matches!(item, EnvelopeItem::ItemContainer(ItemContainer::Logs(_)))));
        assert!(envelope
            .items()
            .any(|item| matches!(item, EnvelopeItem::Attachment(_))));
    }

    /// Without a shared trace Sentry files the logs on their own rather than
    /// against the issue.
    #[test]
    fn the_logs_share_the_events_trace() {
        let dir = tempfile::tempdir().unwrap();
        let (state, recorder) = plugin(dir.path().to_path_buf());

        send(&state, Event::default(), vec![log_saying("connecting")]);

        let envelope = recorder.only();
        let event_trace = envelope.event().unwrap().contexts.get("trace").unwrap();
        let sentry::protocol::Context::Trace(event_trace) = event_trace else {
            panic!("no trace context on the event");
        };
        let logs = envelope
            .items()
            .find_map(|item| match item {
                EnvelopeItem::ItemContainer(ItemContainer::Logs(logs)) => Some(logs),
                _ => None,
            })
            .expect("no logs container");
        assert!(logs
            .iter()
            .all(|log| log.trace_id == Some(event_trace.trace_id)));
    }

    #[test]
    fn names_the_attachment_after_the_newest_log() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dash Chat.log.old"), "a\n").unwrap();
        std::fs::write(dir.path().join("Dash Chat.log"), "b\n").unwrap();

        assert_eq!(log_file_name(dir.path()), "Dash Chat.log");
    }
}
