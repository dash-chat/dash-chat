use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use regex::Regex;

use crate::capture::Breadcrumbs;

pub struct SentryState {
    /// Dropping this disposes the client, so it lives as long as the app — and
    /// dropping it at shutdown is the point: `close` flushes the transport queue.
    guard: sentry::ClientInitGuard,
    pub redact: Vec<Regex>,
    pub breadcrumbs: Arc<Breadcrumbs>,
    /// Arrives from `log_target`, which the app calls after this state exists.
    pub logs_dir: OnceLock<PathBuf>,
}

impl SentryState {
    pub(crate) fn new(
        guard: sentry::ClientInitGuard,
        redact: Vec<Regex>,
        breadcrumbs: Arc<Breadcrumbs>,
    ) -> Self {
        Self {
            guard,
            redact,
            breadcrumbs,
            logs_dir: OnceLock::new(),
        }
    }

    pub fn client(&self) -> &sentry::Client {
        &self.guard
    }
}
