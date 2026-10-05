//! The process's one `log` logger. `log` takes a logger once per process, and
//! on Android the push service and the app share one: a push that finds the app
//! closed starts the process with no app in it, and the app can come up in that
//! same process later. So whichever comes first installs [`ProcessLogger`], and
//! the app swaps its full logger in when it arrives.

use std::sync::{OnceLock, RwLock};

use log::{LevelFilter, Log, Metadata, Record};
use tauri::AppHandle;

use crate::filesystem::FileSystem;

/// The log file in `logs/`: the name `tauri_plugin_log` gives it after the
/// product, spelled out so that a process with no app to ask can append to it.
const LOG_FILE_NAME: &str = "Dash Chat";

/// The log file's size before the app rotates it.
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

const DEFAULT_LEVEL: LevelFilter = LevelFilter::Warn;

const LEVELS: &[(&str, LevelFilter)] = &[
    ("dashchat_node", LevelFilter::Debug),
    ("dashchat_utils", LevelFilter::Debug),
    ("p2panda_net::discovery", LevelFilter::Debug),
    ("p2panda_net::gossip", LevelFilter::Debug),
    ("p2panda_net::iroh_mdns", LevelFilter::Debug),
    ("mailbox_client", LevelFilter::Debug),
    ("mailbox_server", LevelFilter::Debug),
    ("mailbox_local_server", LevelFilter::Debug),
    ("local_hub_discovery", LevelFilter::Debug),
    ("network_watch", LevelFilter::Debug),
    ("tauri_app_lib", LevelFilter::Debug), // dash-chat crate
    ("webview", LevelFilter::Debug),       // JS console.* forwarded via @tauri-apps/plugin-log
];

/// Swappable because the app's logger cannot exist before the app does: its Sentry
/// target needs the `AppHandle`, and its rotating file is private to `tauri_plugin_log`.
struct ProcessLogger {
    inner: RwLock<Box<dyn Log>>,
}

impl Log for ProcessLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.inner.read().is_ok_and(|inner| inner.enabled(metadata))
    }

    fn log(&self, record: &Record) {
        if let Ok(inner) = self.inner.read() {
            inner.log(record);
        }
    }

    fn flush(&self) {
        if let Ok(inner) = self.inner.read() {
            inner.flush();
        }
    }
}

static PROCESS_LOGGER: OnceLock<ProcessLogger> = OnceLock::new();

/// Install `logger` as the process's unless one already is, handing it back
/// then. A push racing the app's startup must not replace the app's logger.
fn install_first(
    logger: Box<dyn Log>,
    max_level: LevelFilter,
) -> anyhow::Result<Option<Box<dyn Log>>> {
    let mut logger = Some(logger);
    let process_logger = PROCESS_LOGGER.get_or_init(|| ProcessLogger {
        inner: RwLock::new(logger.take().expect("taken once")),
    });
    if logger.is_none() {
        log::set_logger(process_logger)?;
        log::set_max_level(max_level);
    }
    Ok(logger)
}

/// Make `logger` the process's, taking over from one a push installed.
fn take_over(logger: Box<dyn Log>, max_level: LevelFilter) -> anyhow::Result<()> {
    let Some(logger) = install_first(logger, max_level)? else {
        return Ok(());
    };
    let process_logger = PROCESS_LOGGER.get().expect("installed above");
    *process_logger
        .inner
        .write()
        .map_err(|_| anyhow::anyhow!("the process logger is poisoned"))? = logger;
    log::set_max_level(max_level);
    Ok(())
}

/// The app's logger. Takes over from whatever a push started the process with.
pub(crate) fn install_app_logger(handle: &AppHandle) -> anyhow::Result<()> {
    let fs = FileSystem::new(handle)?;

    let builder = LEVELS
        .iter()
        .fold(
            tauri_plugin_log::Builder::default().level(DEFAULT_LEVEL),
            |builder, (target, level)| builder.level_for(*target, *level),
        )
        .format(|out, message, _record| out.finish(format_args!("{message}")))
        .clear_targets()
        .max_file_size(MAX_FILE_SIZE as u128)
        // The default, `KeepOne`, deletes the log on every rotation, so a report
        // sent just after one carries almost nothing. ~50 MB in all.
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(9))
        .targets(targets(handle, &fs));

    let error_reporting_dir = fs.error_reporting_dir();
    if let Some(config) = crate::sentry::config(fs.logs_dir(), error_reporting_dir.clone()) {
        std::fs::create_dir_all(&error_reporting_dir)?;
        handle.plugin(tauri_plugin_sentry_reporting::init(config))?;
    }

    let (log_plugin, max_level, logger) = builder.split(handle)?;
    take_over(logger, max_level)?;
    handle.plugin(log_plugin)?;

    Ok(())
}

/// Stdout (logcat on Android), the log file, error reporting and, on iOS,
/// os_log.
fn targets(handle: &AppHandle, fs: &FileSystem) -> Vec<tauri_plugin_log::Target> {
    // Only iOS pushes to it, below.
    #[cfg_attr(not(target_os = "ios"), allow(unused_mut))]
    let mut targets = vec![
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout).format(format_record),
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
            path: fs.logs_dir(),
            file_name: Some(LOG_FILE_NAME.to_string()),
        })
        .format(format_record),
        tauri_plugin_sentry_reporting::log_target(handle),
    ];
    #[cfg(target_os = "ios")]
    targets.push(ios::device_console_target());
    targets
}

#[cfg(target_os = "android")]
pub(crate) mod android {
    use super::*;

    /// The logger of a process a push started with the app closed: logcat and
    /// the app's log file, which the app keeps appending to (and rotates) once it
    /// comes up in this process.
    pub(crate) fn install_push_logger(app_root_dir: &std::path::Path) -> anyhow::Result<()> {
        let logs_dir = FileSystem::from_app_root_dir(app_root_dir.to_path_buf())?.logs_dir();
        std::fs::create_dir_all(&logs_dir)?;
        let dispatch = LEVELS.iter().fold(
            tauri_plugin_log::fern::Dispatch::new()
                .format(format_record)
                .level(DEFAULT_LEVEL),
            |dispatch, (target, level)| dispatch.level_for(*target, *level),
        );
        let (max_level, logger) = dispatch
            .chain(tauri_plugin_log::fern::Output::call(android_logger::log))
            .chain(capped_log_file(&logs_dir)?)
            .into_log();
        install_first(logger, max_level)?;
        Ok(())
    }

    /// The app's log file, moved aside first once it outgrows what the app
    /// would have rotated it at: only the app rotates it, and a device that
    /// gets pushes but is rarely opened would otherwise grow it without end.
    fn capped_log_file(logs_dir: &std::path::Path) -> anyhow::Result<std::fs::File> {
        let path = logs_dir.join(format!("{LOG_FILE_NAME}.log"));
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_FILE_SIZE) {
            std::fs::rename(
                &path,
                logs_dir.join(format!("{LOG_FILE_NAME}.previous.log")),
            )?;
        }
        Ok(tauri_plugin_log::fern::log_file(path)?)
    }
}

#[cfg(target_os = "ios")]
pub(crate) mod ios {
    use super::*;

    /// The notification service extension's logger. It is a process of its own,
    /// so it keeps its own log file.
    pub(crate) fn install_push_extension_logger(log_path: &std::path::Path) -> anyhow::Result<()> {
        let os_logger = oslog::OsLogger::new("studio.darksoil.dashchat.PushNotificationsExtension")
            .level_filter(LevelFilter::Debug);
        let dispatch = LEVELS.iter().fold(
            tauri_plugin_log::fern::Dispatch::new()
                .format(format_record)
                .level(DEFAULT_LEVEL),
            |dispatch, (target, level)| dispatch.level_for(*target, *level),
        );
        let (max_level, logger) = dispatch
            .chain(tauri_plugin_log::fern::log_file(log_path)?)
            .chain(tauri_plugin_log::fern::Output::call(move |record| {
                os_logger.log(record)
            }))
            .into_log();
        install_first(logger, max_level)?;
        Ok(())
    }

    /// os_log is the only log channel that leaves an iOS device, so it is what any
    /// on-device debugging reads. Redacted, unlike the other targets: the unified
    /// log is swept into sysdiagnose archives, which users hand to Apple and attach
    /// to bug reports, and nothing sensitive may leave that way.
    pub(super) fn device_console_target() -> tauri_plugin_log::Target {
        let logger =
            oslog::OsLogger::new("studio.darksoil.dashchat").level_filter(LevelFilter::Debug);
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Dispatch(
            tauri_plugin_log::fern::Dispatch::new().chain(Box::new(logger) as Box<dyn Log>),
        ))
        .format(format_redacted_record)
    }

    fn format_redacted_record(
        out: tauri_plugin_log::fern::FormatCallback,
        message: &std::fmt::Arguments,
        record: &Record,
    ) {
        let redacted = tauri_plugin_sentry_reporting::redact(
            &crate::redaction::REDACTION_REGEXES,
            &message.to_string(),
        );
        format_record(out, &format_args!("{redacted}"), record);
    }
}

pub(crate) fn format_record(
    out: tauri_plugin_log::fern::FormatCallback,
    message: &std::fmt::Arguments,
    record: &log::Record,
) {
    let format =
        time::macros::format_description!("[[[year]-[month]-[day]][[[hour]:[minute]:[second]]");
    let args = if let (Some(file), Some(line)) = (record.file(), record.line()) {
        format_args!(
            "{}[{} {}:{}][{}] {}",
            tauri_plugin_log::TimezoneStrategy::UseUtc
                .get_now()
                .format(&format)
                .unwrap(),
            record.target(),
            file.to_string(),
            line.to_string(),
            record.level(),
            message
        )
    } else {
        format_args!(
            "{}[{}][{}] {}",
            tauri_plugin_log::TimezoneStrategy::UseUtc
                .get_now()
                .format(&format)
                .unwrap(),
            record.target(),
            record.level(),
            message
        )
    };
    out.finish(args)
}
