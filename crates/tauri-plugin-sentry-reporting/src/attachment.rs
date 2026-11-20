use std::path::Path;

use regex::Regex;
use sentry::protocol::Attachment;

use crate::redaction;

const MAX_BYTES: usize = 10 * 1024 * 1024;

pub(crate) async fn build_logs_attachments(
    patterns: &[Regex],
    logs_dirs: &[(&str, &Path)],
) -> Vec<Attachment> {
    let patterns = patterns.to_vec();
    let logs_dirs: Vec<(String, std::path::PathBuf)> = logs_dirs
        .iter()
        .map(|(name, path)| (name.to_string(), path.to_path_buf()))
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        logs_dirs
            .iter()
            .filter_map(|(filename, dir)| redacted_log(&patterns, filename, dir))
            .collect()
    })
    .await
    .ok()
    .unwrap_or_default()
}

fn redacted_log(patterns: &[Regex], filename: &str, logs_dir: &Path) -> Option<Attachment> {
    match redaction::redacted_log_tail(patterns, logs_dir, MAX_BYTES) {
        Ok(text) => Some(Attachment {
            buffer: text.into_bytes(),
            filename: filename.to_owned(),
            content_type: Some("text/plain".into()),
            ty: None,
        }),
        Err(err) => {
            log::warn!("sentry-reporting: could not attach the log from {filename}: {err}");
            None
        }
    }
}
