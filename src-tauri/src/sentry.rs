use std::path::PathBuf;

use tauri_plugin_sentry_reporting::NamedLogDir;

use crate::redaction::REDACTION_REGEXES;

/// `SENTRY_DSN` is set by CI at build time — one DSN for every environment, with
/// `ENV` telling them apart in Sentry. Absent means nothing is reported; logging
/// is unaffected.
pub fn config(
    logs_dir: PathBuf,
    data_dir: PathBuf,
) -> Option<tauri_plugin_sentry_reporting::Config> {
    // Parsed here rather than in the plugin so that registering it cannot fail.
    let dsn = option_env!("SENTRY_DSN")
        .filter(|dsn| !dsn.is_empty())?
        .parse()
        .expect("build.rs fails the build on a SENTRY_DSN that does not parse");

    // Sentry keys regression detection off the release, so identify the build.
    let release = match crate::device_info::build::short_git_sha() {
        Some(sha) => format!("dash-chat@{}+{sha}", env!("CARGO_PKG_VERSION")),
        None => format!("dash-chat@{}", env!("CARGO_PKG_VERSION")),
    };

    #[allow(unused_mut)]
    let mut extra_logs_dirs: Vec<NamedLogDir> = Vec::new();
    #[cfg(target_os = "ios")]
    {
        extra_logs_dirs.push(NamedLogDir {
            name: "notification-service.log".into(),
            dir: data_dir.join("logs-nse"),
        });
    }

    Some(tauri_plugin_sentry_reporting::Config {
        dsn,
        release,
        // The same `ENV` that selects the dotenv file, so Sentry's environment
        // lines up with how the build was produced.
        environment: option_env!("ENV").unwrap_or("development").to_string(),
        redact: REDACTION_REGEXES.clone(),
        logs_dir,
        extra_logs_dirs,
        data_dir,
    })
}
