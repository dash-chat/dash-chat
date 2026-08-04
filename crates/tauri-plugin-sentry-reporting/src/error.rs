use sentry::protocol::{Event, Exception, Level};
use serde::Deserialize;

use crate::envelope;
use crate::state::Sentry;

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
    if let Some(envelope) = envelope::build_envelope(&state, event, logs) {
        state.transport.send(envelope).await;
    }
    Ok(())
}
