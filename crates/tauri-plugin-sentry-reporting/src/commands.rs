use std::path::Path;
use std::sync::Arc;

use sentry::protocol::{Attachment, EnvelopeItem, Event, Exception, Level};
use sentry::Envelope;
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::SentryState;
use crate::{client, redaction};

const MAX_ATTACHED_LOG_BYTES: usize = 1024 * 1024;

/// The thrown value, split into the parts Sentry needs.
#[derive(Debug, Deserialize)]
pub struct ReportedError {
    pub name: String,
    pub message: String,
    pub stack: Option<String>,
}

#[tauri::command]
pub(crate) async fn send_error_report<R: Runtime>(
    app: AppHandle<R>,
    message: String,
    error: Option<ReportedError>,
) -> Result<(), String> {
    // Registering the plugin always manages the state, and this command only
    // exists when it was registered.
    let state = app.state::<Arc<SentryState>>().inner().clone();

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

    // Reading a megabyte of log and running the patterns over it — and over the
    // event itself — is blocking and CPU-bound, so it stays off the async worker.
    tauri::async_runtime::spawn_blocking(move || send(&state, event))
        .await
        .map_err(|err| format!("Report task failed: {err}"))
}

/// Enriches, redacts and transmits the report. The only path to the network.
fn send(state: &SentryState, event: Event<'static>) {
    let event = client::enrich(event, state.client());

    let event = match redaction::redact_event(&state.redact, event) {
        Ok(event) => event,
        Err(err) => {
            log::error!("sentry-reporting: dropping unredactable report: {err}");
            return;
        }
    };

    let mut envelope: Envelope = event.into();
    if let Some(attachment) = read_log_attachment(state) {
        envelope.add_item(EnvelopeItem::Attachment(attachment));
    }
    state.client().send_envelope(envelope);
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

    #[test]
    fn names_the_attachment_after_the_newest_log() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dash Chat.log.old"), "a\n").unwrap();
        std::fs::write(dir.path().join("Dash Chat.log"), "b\n").unwrap();

        assert_eq!(log_file_name(dir.path()), "Dash Chat.log");
    }

    #[test]
    fn falls_back_when_the_directory_has_no_logs() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(log_file_name(dir.path()), "app.log");
    }
}
