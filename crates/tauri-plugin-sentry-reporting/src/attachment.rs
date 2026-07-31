use std::path::Path;

use regex::Regex;
use sentry::protocol::Attachment;

use crate::redaction;
use crate::state::SentryState;

const MAX_BYTES: usize = 1024 * 1024;

pub(crate) async fn read(state: &SentryState) -> Option<Attachment> {
    let patterns = state.redact.clone();
    let logs_dir = state.logs_dir.clone();
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

/// Names the attachment after the log it was tailed from, e.g. `Dash Chat.log`.
fn log_file_name(logs_dir: &Path) -> String {
    redaction::list_log_files_oldest_first(logs_dir)
        .ok()
        .and_then(|files| Some(files.last()?.file_name()?.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app.log".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_attachment_after_the_newest_log() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dash Chat.log.old"), "a\n").unwrap();
        std::fs::write(dir.path().join("Dash Chat.log"), "b\n").unwrap();

        assert_eq!(log_file_name(dir.path()), "Dash Chat.log");
    }
}
