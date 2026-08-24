use sentry::protocol::{EnvelopeItem, Event, Exception, Level};
use serde::Deserialize;

use crate::state::{SendOutcome, Sentry};
use crate::{attachment, envelope};

/// The thrown value, split into the parts Sentry needs.
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
) -> Result<SendOutcome, String> {
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
    // A report `before_send` dropped is not waiting on anything.
    let Some(mut envelope) = envelope::build_envelope(&state, event, logs) else {
        return Ok(SendOutcome::Sent);
    };
    if let Some(log_file) = attachment::build_logs_attachment(&state.redact, &state.logs_dir).await
    {
        envelope.add_item(EnvelopeItem::Attachment(log_file));
    }
    state.send(envelope).await.map_err(|err| err.to_string())
}
