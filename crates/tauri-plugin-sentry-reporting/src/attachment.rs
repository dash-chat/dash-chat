use std::path::Path;

use regex::Regex;
use sentry::protocol::Attachment;

use crate::redaction;

const MAX_BYTES: usize = 1024 * 1024;

pub(crate) async fn build_logs_attachment(
    patterns: &[Regex],
    logs_dir: &Path,
) -> Option<Attachment> {
    let patterns = patterns.to_vec();
    let logs_dir = logs_dir.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || redacted_log(&patterns, &logs_dir))
        .await
        .ok()
        .flatten()
}

fn redacted_log(patterns: &[Regex], logs_dir: &Path) -> Option<Attachment> {
    match redaction::redacted_log_tail(patterns, logs_dir, MAX_BYTES) {
        Ok(text) => Some(Attachment {
            buffer: text.into_bytes(),
            filename: log_file_name(logs_dir),
            content_type: Some("text/plain".into()),
            ty: None,
        }),
        Err(err) => {
            log::warn!("sentry-reporting: could not attach the log: {err}");
            None
        }
    }
}

fn log_file_name(logs_dir: &Path) -> String {
    redaction::list_log_files_oldest_first(logs_dir)
        .ok()
        .and_then(|files| Some(files.last()?.file_name()?.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app.log".to_owned())
}
