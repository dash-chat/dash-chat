//! Sentry error reporting on a strict rule: **nothing leaves the device unless
//! the user explicitly presses Send.**
//!
//! The SDK is configured so it cannot transmit anything by itself ([`client`]);
//! a report is assembled and sent from a user-initiated command and nowhere else.
//!
//! ```ignore
//! // once `tauri-plugin-log` owns the log files, so a report can attach them:
//! if let Some(config) = sentry::config(fs.logs_dir()) {
//!     handle.plugin(tauri_plugin_sentry_reporting::init(config))?;
//! }
//! ```

mod client;
mod commands;
mod redaction;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub use redaction::{redact, redacted_log_tail};
pub use sentry::types::Dsn;

use crate::state::SentryState;

pub struct Config {
    /// Parsed by the caller, so registering the plugin cannot fail.
    pub dsn: Dsn,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Redaction patterns to be applied to logs before sending them
    pub redact: Vec<regex::Regex>,
    /// Where the log files a report attaches live.
    pub logs_dir: PathBuf,
}

/// Register only when the app has a DSN. The frontend gates its report action on
/// the same build-time flag, so it never offers what this cannot deliver.
///
/// Register once the log files have an owner: [`Config::logs_dir`] is resolved
/// from an `AppHandle`, and a report is only as useful as the log it carries.
pub fn init<R: Runtime>(config: Config) -> TauriPlugin<R> {
    let state = Arc::new(SentryState::new(
        sentry::init(client::options(&config)),
        config.redact,
        config.logs_dir,
    ));

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![commands::send_error_report])
        .setup(move |app, _api| {
            app.manage(state);
            Ok(())
        })
        .build()
}
