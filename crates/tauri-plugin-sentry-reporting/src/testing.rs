//! Scaffolding shared by the crate's tests.

use std::path::Path;
use std::sync::Arc;

use sentry::protocol::{Log, LogLevel};
use sentry::Envelope;

use crate::state::SentryState;
use crate::Config;

pub(crate) fn config(dir: &Path) -> Config {
    Config {
        dsn: "https://key@example.invalid/1".parse().unwrap(),
        release: "dash-chat@0.0.0".into(),
        environment: "test".into(),
        redact: vec![regex::Regex::new(r"secret-\w+").unwrap()],
        logs_dir: dir.to_path_buf(),
        data_dir: dir.to_path_buf(),
    }
}

pub(crate) fn log_saying(body: &str) -> Log {
    Log {
        level: LogLevel::Info,
        body: body.to_owned(),
        trace_id: None,
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        severity_number: None,
        attributes: Default::default(),
    }
}

/// A plugin wired exactly as `init` wires one.
pub(crate) fn plugin(dir: &Path) -> Arc<SentryState> {
    SentryState::new(config(dir))
}

/// Parses a stored entry, which the outbox deliberately reads back verbatim.
pub(crate) fn parsed(envelope: &Envelope) -> Envelope {
    let mut bytes = Vec::new();
    envelope.to_writer(&mut bytes).unwrap();
    Envelope::from_slice(&bytes).expect("the stored envelope does not parse")
}

/// A feedback report as `feedback::feedback_envelope` builds one: raw bytes
/// whose `feedback` item type the SDK's parser does not know.
pub(crate) fn feedback_envelope() -> Envelope {
    let payload = r#"{"event_id":"20ded16f15f0407cb799852e85820b8b","platform":"native","contexts":{"feedback":{"message":"it broke"}}}"#;
    let bytes = format!(
        "{{\"event_id\":\"20ded16f15f0407cb799852e85820b8b\"}}\n{{\"type\":\"feedback\",\"length\":{}}}\n{payload}\n",
        payload.len()
    );
    Envelope::from_bytes_raw(bytes.into_bytes()).unwrap()
}
