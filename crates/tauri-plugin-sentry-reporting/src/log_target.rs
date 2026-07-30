use std::path::PathBuf;
use std::sync::Arc;

use sentry::protocol::{Breadcrumb, Level};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::{Target, TargetKind, WEBVIEW_TARGET};

use crate::capture::Captured;
use crate::state::SentryState;

/// Feeds Sentry breadcrumbs, and records where the log files live for the
/// report attachment.
///
/// A target rather than a logger because `log` allows exactly one global logger,
/// and the app owns it.
pub fn log_target<R: Runtime>(app: &AppHandle<R>, logs_dir: PathBuf) -> Target {
    let captured = match app.try_state::<Arc<SentryState>>() {
        Some(state) => {
            let _ = state.logs_dir.set(logs_dir);
            Some(state.captured.clone())
        }
        None => None,
    };

    let dispatch =
        fern::Dispatch::new().chain(Box::new(BreadcrumbLogger { captured }) as Box<dyn log::Log>);

    // No allowlist needed: records only arrive after the app's own `.level()` /
    // `.level_for()` filters. Webview records are excluded because the injected
    // browser SDK already breadcrumbs console output.
    Target::new(TargetKind::Dispatch(dispatch)).filter(|metadata| {
        metadata.level() <= log::Level::Info && !metadata.target().starts_with(WEBVIEW_TARGET)
    })
}

struct BreadcrumbLogger {
    captured: Option<Arc<Captured>>,
}

impl log::Log for BreadcrumbLogger {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        self.captured.is_some()
    }

    fn log(&self, record: &log::Record<'_>) {
        let Some(captured) = &self.captured else {
            return;
        };
        captured.add_breadcrumb(Breadcrumb {
            ty: "log".into(),
            category: Some(record.target().to_owned()),
            level: to_sentry_level(record.level()),
            message: Some(record.args().to_string()),
            ..Default::default()
        });
    }

    fn flush(&self) {}
}

fn to_sentry_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
    }
}
