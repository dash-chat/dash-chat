//! Sentry error reporting on a strict rule: **nothing leaves the device unless
//! the user explicitly presses Send.**
//!
//! Capture is local-only ([`capture`]); egress happens from a user-initiated
//! command and nowhere else.
//!
//! ```ignore
//! builder = builder.plugin(tauri_plugin_sentry_reporting::init(sentry::config()));
//!
//! .targets([
//!     Target::new(TargetKind::Stdout),
//!     Target::new(TargetKind::Folder { path: fs.logs_dir(), file_name: None }),
//!     tauri_plugin_sentry_reporting::log_target(handle, fs.logs_dir()),
//! ])
//! ```

mod capture;
mod commands;
mod log_target;
mod redaction;
mod state;

use std::sync::Arc;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub use log_target::log_target;
pub use redaction::{redact, redacted_log_tail};

pub struct Config {
    pub dsn: String,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Redaction patterns to be applied to logs before sending them
    pub redact: Vec<regex::Regex>,
}

/// `None` disables reporting while keeping the command registered, so callers
/// need no conditional wiring.
///
/// `sentry::init` runs here, at builder time, before any thread inherits a `Hub`.
pub fn init<R: Runtime>(config: Option<Config>) -> TauriPlugin<R> {
    let state = config.and_then(start).map(Arc::new);

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![commands::send_error_report])
        .setup(move |app, _api| {
            if let Some(state) = state {
                app.manage(state);
            }
            Ok(())
        })
        .build()
}

/// `None` when the DSN is missing or unparseable, which disables reporting and
/// leaves the command a no-op.
fn start(config: Config) -> Option<state::SentryState> {
    let captured = Arc::new(capture::Captured::new());
    let options = capture::client_options(&config, captured.clone())
        .inspect_err(|err| log::error!("sentry-reporting: disabled: {err}"))
        .ok()?;

    Some(state::SentryState::new(
        sentry::init(options),
        config.redact,
        captured,
    ))
}
