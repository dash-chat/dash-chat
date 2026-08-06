use std::io::Write;

use anyhow::Context;
use sentry::protocol::{Attachment, Event, Level};
use sentry::Envelope;
use serde::Deserialize;
use serde_json::json;

use crate::attachment;
use crate::state::{Sentry, SentryState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feedback {
    pub reason: String,
    pub message: String,
    pub screenshot: Option<Screenshot>,
    pub include_logs: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[tauri::command]
pub(crate) async fn send_feedback(state: Sentry<'_>, feedback: Feedback) -> Result<(), String> {
    let envelope = build_feedback(&state, feedback)
        .await
        .map_err(|err| err.to_string())?;
    state.transport.send(envelope);
    Ok(())
}

async fn build_feedback(state: &SentryState, feedback: Feedback) -> anyhow::Result<Envelope> {
    let mut event = Event {
        message: Some(feedback.message),
        level: Level::Info,
        ..Default::default()
    };
    event.tags.insert("reason".into(), feedback.reason);
    let event = state
        .client
        .prepare_event(event, None)
        .context("the feedback could not be redacted")?;

    let mut attachments = Vec::new();
    if let Some(screenshot) = feedback.screenshot {
        attachments.push(Attachment {
            buffer: screenshot.bytes,
            filename: screenshot.name,
            content_type: Some(screenshot.content_type),
            ty: None,
        });
    }
    if feedback.include_logs {
        attachments.extend(attachment::build_logs_attachment(&state.redact, &state.logs_dir).await);
    }

    feedback_envelope(event, &attachments)
}

/// Written out by hand because the Rust SDK has no feedback item
fn feedback_envelope(
    mut event: Event<'static>,
    attachments: &[Attachment],
) -> anyhow::Result<Envelope> {
    let event_id = event.event_id;
    let message = event.message.take().unwrap_or_default();
    let mut payload = serde_json::to_value(event)?;
    payload["contexts"]["feedback"] = json!({ "message": message });
    let payload = serde_json::to_vec(&payload)?;

    let mut bytes = Vec::new();
    writeln!(bytes, r#"{{"event_id":"{}"}}"#, event_id.simple())?;
    writeln!(bytes, r#"{{"type":"feedback","length":{}}}"#, payload.len())?;
    bytes.extend_from_slice(&payload);
    writeln!(bytes)?;
    for attachment in attachments {
        attachment.to_writer(&mut bytes)?;
        writeln!(bytes)?;
    }
    Ok(Envelope::from_bytes_raw(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use tauri::async_runtime::block_on;

    use crate::testing::plugin;

    fn sent(state: &SentryState, include_logs: bool, screenshot: bool) -> String {
        let feedback = Feedback {
            reason: "bug".into(),
            message: "it dropped my photo, token secret-abc123".into(),
            screenshot: screenshot.then(|| Screenshot {
                name: "screenshot.jpg".into(),
                content_type: "image/jpeg".into(),
                bytes: b"\xff\xd8\xff".to_vec(),
            }),
            include_logs,
        };
        let envelope = block_on(build_feedback(state, feedback)).expect("no envelope built");
        let mut bytes = Vec::new();
        envelope.to_writer(&mut bytes).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[test]
    fn a_feedback_carries_only_what_was_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.log"), "a line\n").unwrap();
        let (state, _) = plugin(dir.path());

        let both = sent(&state, true, true);
        assert!(
            both.contains(r#"{"type":"feedback","length":"#),
            "got: {both}"
        );
        assert!(
            both.contains(r#""feedback":{"message":"it dropped my photo, token [REDACTED]"}"#),
            "got: {both}"
        );
        // Required of every feedback, and what tells Sentry which build it is.
        assert!(both.contains(r#""platform":"native""#), "got: {both}");
        assert!(
            both.contains(r#""release":"dash-chat@0.0.0""#),
            "got: {both}"
        );
        assert!(both.contains(r#""reason":"bug""#), "got: {both}");
        assert!(
            both.contains(r#""filename":"screenshot.jpg""#),
            "got: {both}"
        );
        assert!(both.contains("a line"), "got: {both}");

        let neither = sent(&state, false, false);
        assert!(
            !neither.contains(r#""type":"attachment""#),
            "got: {neither}"
        );
    }

    #[test]
    fn a_missing_log_is_no_attachment() {
        let (state, _) = plugin(Path::new("/nonexistent"));

        let sent = sent(&state, true, false);

        assert!(
            sent.contains(r#"{"type":"feedback","length":"#),
            "got: {sent}"
        );
        assert!(!sent.contains(r#""type":"attachment""#), "got: {sent}");
    }
}
