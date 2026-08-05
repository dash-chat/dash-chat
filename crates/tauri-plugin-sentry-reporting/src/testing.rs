//! Scaffolding shared by the crate's tests.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sentry::protocol::{Envelope, Log, LogLevel};
use sentry::Transport;

use crate::state::SentryState;
use crate::transport::UserInitiatedTransport;
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

pub(crate) fn recording_transport() -> (UserInitiatedTransport, Arc<TestRecorderTransport>) {
    let recorder = Arc::new(TestRecorderTransport::default());
    let transport = UserInitiatedTransport::default();
    let _ = transport.inner.set(recorder.clone());
    (transport, recorder)
}

/// A plugin wired exactly as `init` wires one, but recording instead of sending.
pub(crate) fn plugin(dir: &Path) -> (Arc<SentryState>, Arc<TestRecorderTransport>) {
    let (transport, recorder) = recording_transport();
    (SentryState::new(config(dir), Arc::new(transport)), recorder)
}

#[derive(Default)]
pub(crate) struct TestRecorderTransport {
    envelopes: Mutex<Vec<Envelope>>,
    drained: AtomicBool,
}

impl TestRecorderTransport {
    pub(crate) fn sent(&self) -> Vec<Envelope> {
        self.envelopes.lock().unwrap().clone()
    }

    pub(crate) fn drained(&self) -> bool {
        self.drained.load(Ordering::Relaxed)
    }
}

impl Transport for TestRecorderTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.envelopes.lock().unwrap().push(envelope);
    }

    /// `shutdown` defaults to this, so it records both ways of draining.
    fn flush(&self, _timeout: Duration) -> bool {
        self.drained.store(true, Ordering::Relaxed);
        true
    }
}
