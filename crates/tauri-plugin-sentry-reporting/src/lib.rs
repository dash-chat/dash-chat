//! Sentry error reporting on a strict rule: **nothing leaves the device unless
//! the user explicitly presses Send.**
//!
//! The SDK is configured so it cannot transmit anything by itself ([`capture`]);
//! a report is assembled and sent from a user-initiated command and nowhere else.
//!
//! ```ignore
//! if let Some(config) = sentry::config() {
//!     builder = builder.plugin(tauri_plugin_sentry_reporting::init(config));
//! }
//!
//! // once tauri-plugin-log is registered, so a report can attach the log:
//! tauri_plugin_sentry_reporting::set_logs_dir(handle, fs.logs_dir());
//! ```

mod capture;
mod commands;
mod redaction;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime};

pub use redaction::{redact, redacted_log_tail};
pub use sentry::types::{Dsn, ParseDsnError};

use crate::state::SentryState;

pub struct Config {
    /// Parsed by the caller, so registering the plugin cannot fail.
    pub dsn: Dsn,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Redaction patterns to be applied to logs before sending them
    pub redact: Vec<regex::Regex>,
}

/// Register only when the app has a DSN. The frontend gates its report action on
/// the same build-time flag, so it never offers what this cannot deliver.
///
/// `sentry::init` runs here, at builder time, before any thread inherits a `Hub`.
pub fn init<R: Runtime>(config: Config) -> TauriPlugin<R> {
    let state = Arc::new(state::SentryState::new(
        sentry::init(capture::client_options(&config)),
        config.redact,
    ));

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![commands::send_error_report])
        .setup(move |app, _api| {
            app.manage(state);
            Ok(())
        })
        .build()
}

/// Tells the plugin where to find the log files a report attaches. Call once
/// `tauri-plugin-log` owns them. Inert when reporting is disabled.
pub fn set_logs_dir<R: Runtime>(app: &AppHandle<R>, logs_dir: PathBuf) {
    if let Some(state) = app.try_state::<Arc<SentryState>>() {
        let _ = state.logs_dir.set(logs_dir);
    }
}
