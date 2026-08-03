//! Sentry error reporting on a strict rule: **nothing leaves the device unless
//! the user explicitly presses Send.**
//!
//! The SDK captures freely; [`transport::ConsentGate`] is what holds every
//! envelope until a report is sent.

mod attachment;
mod client;
mod crash;
mod envelope;
mod error;
mod logs;
mod redaction;
mod state;
#[cfg(test)]
mod testing;
mod transport;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub use logs::log_target;
pub use redaction::{redact, redacted_log_tail};
pub use sentry::types::Dsn;

use crate::state::SentryState;
use crate::transport::ConsentGate;

pub struct Config {
    /// Parsed by the caller, so registering the plugin cannot fail.
    pub dsn: Dsn,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Applied to everything on its way off the device.
    pub redact: Vec<regex::Regex>,
    /// Where the log files a report attaches live.
    pub logs_dir: PathBuf,
    /// This crate's own folder, holding a crash kept for the next launch.
    pub data_dir: PathBuf,
}

pub fn init<R: Runtime>(config: Config) -> TauriPlugin<R> {
    let state = SentryState::new(config, Arc::new(ConsentGate::default()));
    crash::install_panic_hook(Arc::downgrade(&state));

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![
            error::send_error_report,
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
