use std::path::PathBuf;

use regex::Regex;

pub struct SentryState {
    /// Dropping this disposes the client, so it lives as long as the app — and
    /// dropping it at shutdown is the point: `close` flushes the transport queue.
    guard: sentry::ClientInitGuard,
    pub redact: Vec<Regex>,
    pub logs_dir: PathBuf,
}

impl SentryState {
    pub(crate) fn new(
        guard: sentry::ClientInitGuard,
        redact: Vec<Regex>,
        logs_dir: PathBuf,
    ) -> Self {
        Self {
            guard,
            redact,
            logs_dir,
        }
    }

    pub fn client(&self) -> &sentry::Client {
        &self.guard
    }
}
