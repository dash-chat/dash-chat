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

use std::sync::{Arc, OnceLock};

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub use log_target::log_target;

pub struct Config {
    pub dsn: String,
    /// Identifies the build, e.g. `dash-chat@0.19.13+abc1234`.
    pub release: String,
    pub environment: String,
    /// Applied to everything on its way off the device. The app owns the patterns;
    /// the plugin decides where they apply, which is everywhere.
    pub redact: Vec<regex::Regex>,
}

/// `None` disables reporting while keeping the command registered, so callers
/// need no conditional wiring.
///
/// `sentry::init` runs here, at builder time, before any thread inherits a `Hub`.
pub fn init<R: Runtime>(config: Option<Config>) -> TauriPlugin<R> {
    let started = config.and_then(|config| {
        let captured = Arc::new(capture::Captured::new());
        let options = capture::client_options(&config, captured.clone())
            .inspect_err(|err| log::error!("sentry-reporting: disabled: {err}"))
            .ok()?;
        let guard = sentry::init(options);
        let client = guard.clone();
        Some((config, guard, client, captured))
    });

    Builder::<R>::new("sentry-reporting")
        .invoke_handler(tauri::generate_handler![commands::send_error_report])
        .setup(move |app, _api| {
            let Some((config, guard, client, captured)) = started else {
                return Ok(());
            };

            app.manage(Arc::new(state::SentryState {
                client: client.into(),
                redact: config.redact,
                captured,
                logs_dir: OnceLock::new(),
            }));
            // Dropping the guard disposes the client, so the app owns it for its
            // lifetime. Dropping it at shutdown is the point: `close` flushes
            // whatever is still in the transport queue.
            app.manage(guard);

            Ok(())
        })
        .build()
}
