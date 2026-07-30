use std::sync::Arc;

use sentry::protocol::{Attachment, EnvelopeItem, Event, Exception, Level};
use sentry::Envelope;
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::SentryState;
use crate::{capture, redaction};

const MAX_ATTACHED_LOG_BYTES: usize = 1024 * 1024;
const LOG_ATTACHMENT_NAME: &str = "dashchat.log";

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
    include_log: bool,
) -> Result<(), String> {
    let Some(state) = app.try_state::<Arc<SentryState>>() else {
        // Built without a DSN. Report success rather than surfacing an error the
        // user can do nothing about.
        log::warn!("sentry-reporting: no DSN configured, discarding report");
        return Ok(());
    };
    let state = state.inner().clone();

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
    tauri::async_runtime::spawn_blocking(move || send(&state, event, include_log))
        .await
        .map_err(|err| format!("Report task failed: {err}"))
}

/// Enriches, redacts and transmits the report. The only path to the network.
fn send(state: &SentryState, event: Event<'static>, include_log: bool) {
    let event = capture::enrich(event, state.client(), &state.breadcrumbs);

    let event = match redaction::redact_event(&state.redact, event) {
        Ok(event) => event,
        Err(err) => {
            log::error!("sentry-reporting: dropping unredactable report: {err}");
            return;
        }
    };

    let mut envelope: Envelope = event.into();
    if let Some(attachment) = include_log.then(|| read_log_attachment(state)).flatten() {
        envelope.add_item(EnvelopeItem::Attachment(attachment));
    }
    state.client().send_envelope(envelope);
}

fn read_log_attachment(state: &SentryState) -> Option<Attachment> {
    let logs_dir = state.logs_dir.get()?;
    match redaction::redacted_log_tail(&state.redact, logs_dir, MAX_ATTACHED_LOG_BYTES) {
        Ok(text) => Some(Attachment {
            buffer: text.into_bytes(),
            filename: LOG_ATTACHMENT_NAME.into(),
            content_type: Some("text/plain".into()),
            ty: None,
        }),
        Err(err) => {
            log::warn!("sentry-reporting: could not attach the log: {err}");
            None
        }
    }
}
