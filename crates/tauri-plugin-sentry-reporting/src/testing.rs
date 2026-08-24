//! Scaffolding shared by the crate's tests.

use std::path::Path;
use std::sync::Arc;

use sentry::protocol::{Log, LogLevel};

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
