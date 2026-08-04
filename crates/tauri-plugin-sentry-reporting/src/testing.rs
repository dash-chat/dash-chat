//! Scaffolding shared by the crate's tests.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sentry::protocol::{Envelope, Log, LogLevel};
use sentry::Transport;

use crate::state::SentryState;
use crate::transport::UserInitiatedTransport;
use crate::Config;

pub(crate) fn config(logs_dir: PathBuf) -> Config {
    Config {
        dsn: "https://key@example.invalid/1".parse().unwrap(),
        release: "dash-chat@0.0.0".into(),
        environment: "test".into(),
        redact: vec![regex::Regex::new(r"secret-\w+").unwrap()],
        logs_dir,
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

pub(crate) fn recording_transport(
    config: &Config,
) -> (UserInitiatedTransport, Arc<TestRecorderTransport>) {
    let recorder = Arc::new(TestRecorderTransport::default());
    let transport = UserInitiatedTransport::new(config);
    let _ = transport.inner.set(recorder.clone());
    (transport, recorder)
}

/// A plugin wired exactly as `init` wires one, but recording instead of sending.
pub(crate) fn plugin(logs_dir: PathBuf) -> (Arc<SentryState>, Arc<TestRecorderTransport>) {
    let config = config(logs_dir);
    let (transport, recorder) = recording_transport(&config);
    (SentryState::new(config, Arc::new(transport)), recorder)
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

    pub(crate) fn only(&self) -> Envelope {
        let mut sent = self.sent();
        assert_eq!(sent.len(), 1, "expected one envelope");
        sent.remove(0)
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
