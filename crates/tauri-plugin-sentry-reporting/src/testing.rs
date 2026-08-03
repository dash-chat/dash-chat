//! Scaffolding shared by the crate's tests.

use std::path::Path;
use std::sync::{Arc, Mutex};

use sentry::protocol::{Envelope, Log, LogLevel};
use sentry::Transport;

use crate::state::SentryState;
use crate::transport::ConsentGate;
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

pub(crate) fn recording_gate() -> (ConsentGate, Arc<TestRecorderTransport>) {
    let recorder = Arc::new(TestRecorderTransport::default());
    let gate = ConsentGate::default();
    let _ = gate.inner.set(recorder.clone());
    (gate, recorder)
}

/// A plugin wired exactly as `init` wires one, but recording instead of sending.
pub(crate) fn plugin(dir: &Path) -> (Arc<SentryState>, Arc<TestRecorderTransport>) {
    let (gate, recorder) = recording_gate();
    (SentryState::new(config(dir), Arc::new(gate)), recorder)
}

#[derive(Default)]
pub(crate) struct TestRecorderTransport(Mutex<Vec<Envelope>>);

impl TestRecorderTransport {
    pub(crate) fn sent(&self) -> Vec<Envelope> {
        self.0.lock().unwrap().clone()
    }

    pub(crate) fn only(&self) -> Envelope {
        let mut sent = self.sent();
        assert_eq!(sent.len(), 1, "expected one envelope");
        sent.remove(0)
    }
}

impl Transport for TestRecorderTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.0.lock().unwrap().push(envelope);
    }
}
