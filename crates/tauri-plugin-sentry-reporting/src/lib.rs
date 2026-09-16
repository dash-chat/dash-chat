//! Sentry error reporting on a strict rule: **nothing leaves the device unless
//! the user explicitly presses Send.**
//!
//! The SDK captures freely; nothing it hands the transport is ever sent. A
//! report the user pressed Send on goes to the outbox, which delivers it and
//! keeps it on disk until it can.

mod attachment;
mod client;
mod crash;
mod envelope;
mod error;
mod feedback;
mod logs;
mod outbox;
mod redaction;
mod state;
#[cfg(test)]
mod testing;
mod transport;

use std::path::PathBuf;
use std::sync::Arc;

/// A named log directory to attach to reports as a separate file.
#[derive(Debug, Clone)]
pub struct NamedLogDir {
    /// The attachment filename shown in Sentry.
    pub name: String,
    /// The directory containing `*.log` files.
    pub dir: PathBuf,
}

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub use logs::log_target;
pub use redaction::redact;
pub use sentry::types::Dsn;
pub use state::SendOutcome;

use crate::state::SentryState;

pub struct Config {
    /// Parsed by the caller, so registering the plugin cannot fail.
    pub dsn: Dsn,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Applied to everything on its way off the device.
    pub redact: Vec<regex::Regex>,
    /// Where the main app log files a report attaches live.
    pub logs_dir: PathBuf,
    /// Additional named log directories to include as separate attachments.
    pub extra_logs_dirs: Vec<NamedLogDir>,
    /// This crate's own folder, holding the outbox of reports waiting to go out.
    pub data_dir: PathBuf,
}

pub fn init<R: Runtime>(config: Config) -> TauriPlugin<R> {
    let state = SentryState::new(config);
    crash::install_panic_hook(Arc::downgrade(&state));

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![
            error::send_error_report,
            feedback::send_feedback,
            crash::pending_crash_report,
            crash::send_pending_crash_report,
            crash::discard_pending_crash_report,
        ])
        .setup(move |app, _api| {
            app.manage(state);
            Ok(())
        })
        .build()
}
