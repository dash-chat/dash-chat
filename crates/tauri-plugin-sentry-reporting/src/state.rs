use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use regex::Regex;

use crate::capture::Captured;

pub struct SentryState {
    pub client: Arc<sentry::Client>,
    pub redact: Vec<Regex>,
    pub captured: Arc<Captured>,
    /// Arrives from `log_target`, which the app calls after this state exists.
    pub logs_dir: OnceLock<PathBuf>,
}
