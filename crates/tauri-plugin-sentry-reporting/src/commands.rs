use std::sync::Arc;

use sentry::protocol::{Attachment, EnvelopeItem, Event, Exception, Level};
use sentry::Envelope;
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::redaction;
use crate::state::SentryState;

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

    let mut event = Event {
        message: Some(message),
        level: Level::Error,
        ..Default::default()
    };
    if let Some(error) = error {
        if let Some(stack) = error.stack {
            event.extra.insert("stack".into(), stack.into());
        }
        // Sentry titles and groups by the exception. Leaving the real error in
        // `extra` instead would collapse every unexpected error into one issue
        // named after the generic toast copy — and `extra` is not searchable.
        event.exception = vec![Exception {
            ty: error.name,
            value: Some(error.message),
            ..Default::default()
        }]
        .into();
    }
    // Goes through `before_send`, so it lands in the pending queue enriched with
    // contexts and the breadcrumb trail.
    sentry::capture_event(event);

    let log_attachment = include_log.then(|| read_log_attachment(&state)).flatten();

    for (index, pending) in state.captured.take_pending().into_iter().enumerate() {
        let redacted = redaction::redact_event(&state.redact, pending)
            .map_err(|err| format!("Failed to redact report: {err}"))?;
        let mut envelope: Envelope = redacted.into();
        // Repeating the log per stashed event would blow the size limit.
        if index == 0 {
            if let Some(attachment) = log_attachment.clone() {
                envelope.add_item(EnvelopeItem::Attachment(attachment));
            }
        }
        // Deliberately not `capture_event`: that would re-enter `before_send`
        // and stash the event straight back into the queue.
        //
        // The transport is fire-and-forget, so this reports success whether or
        // not the report lands. A durable queue with a real outcome follows.
        state.client.send_envelope(envelope);
    }

    Ok(())
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
