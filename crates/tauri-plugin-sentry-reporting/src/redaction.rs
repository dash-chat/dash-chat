use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

const PLACEHOLDER: &str = "[REDACTED]";

/// Replaces every match of `patterns` in `text` with `[REDACTED]`.
pub fn redact(patterns: &[Regex], text: &str) -> String {
    let mut out = text.to_owned();
    for re in patterns {
        out = re.replace_all(&out, PLACEHOLDER).into_owned();
    }
    out
}

/// Per leaf, not over the serialized document: several patterns match a whole
/// `"key": "value"` pair and would replace it wholesale, producing invalid JSON.
pub(crate) fn redact_json_leaves(patterns: &[Regex], value: &mut Value) {
    match value {
        Value::String(s) => *s = redact(patterns, s),
        Value::Array(items) => {
            for item in items {
                redact_json_leaves(patterns, item);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                redact_json_leaves(patterns, v);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Round-trips through JSON so every string the value carries is covered — an
/// event's named fields, a log's attributes, and free-form maps alike.
pub(crate) fn redact_serialized<T>(patterns: &[Regex], value: T) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned,
{
    let mut json = serde_json::to_value(value)?;
    redact_json_leaves(patterns, &mut json);
    Ok(serde_json::from_value(json)?)
}

/// Newline-terminated, so the boundary between two files is always a line break.
pub(crate) fn concat_files(paths: &[PathBuf]) -> std::io::Result<String> {
    let mut buf = String::new();
    for path in paths {
        std::fs::File::open(path)?.read_to_string(&mut buf)?;
        if !buf.is_empty() && !buf.ends_with('\n') {
            buf.push('\n');
        }
    }
    Ok(buf)
}

pub(crate) fn last_whole_lines(text: &str, max_bytes: usize) -> &str {
    let start = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .find(|&start| text.len() - start <= max_bytes)
        .unwrap_or(text.len());
    &text[start..]
}

/// Every `*.log` in `dir`, oldest first by mtime; rotation can leave several date-stamped files.
pub(crate) fn list_log_files_oldest_first(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("log"))
        .filter_map(|e| {
            let mtime = e.metadata().and_then(|m| m.modified()).ok()?;
            Some((mtime, e.path()))
        })
        .collect();
    entries.sort_by_key(|(mtime, _)| *mtime);
    Ok(entries.into_iter().map(|(_, p)| p).collect())
}

/// Redacted last `max_bytes` of the logs in `logs_dir`, concatenated oldest first.
pub fn redacted_log_tail(
    patterns: &[Regex],
    logs_dir: &Path,
    max_bytes: usize,
) -> anyhow::Result<String> {
    let files = list_log_files_oldest_first(logs_dir)?;
    if files.is_empty() {
        anyhow::bail!("no log files in {}", logs_dir.display());
    }
    let text = concat_files(&files)?;
    Ok(redact(patterns, last_whole_lines(&text, max_bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::Event;

    fn patterns() -> Vec<Regex> {
        vec![
            Regex::new(r"[0-9a-fA-F]{40,}").unwrap(),
            Regex::new(r#""message"\s*:\s*"[^"]*""#).unwrap(),
            Regex::new(r"/home/[^/\s]+").unwrap(),
        ]
    }

    #[test]
    fn redacts_matches_in_a_plain_string() {
        let hex = "a".repeat(40);
        assert_eq!(redact(&patterns(), &hex), PLACEHOLDER);
    }

    #[test]
    fn leaf_redaction_preserves_structure_and_keys() {
        let mut value = serde_json::json!({
            "message": "hello",
            "nested": { "message": "world", "count": 3, "flag": true, "nil": null },
            "list": ["ok", "b".repeat(40)],
        });
        redact_json_leaves(&patterns(), &mut value);

        assert_eq!(value["nested"]["count"], 3);
        assert_eq!(value["nested"]["flag"], true);
        assert!(value["nested"]["nil"].is_null());
        assert_eq!(value["message"], "hello");
        assert_eq!(value["list"][0], "ok");
        assert_eq!(value["list"][1], PLACEHOLDER);
    }

    #[test]
    fn leaf_redaction_still_catches_json_embedded_in_a_log_line() {
        let mut value = serde_json::json!({
            "line": r#"[INFO] publishing {"message":"secret text"}"#,
        });
        redact_json_leaves(&patterns(), &mut value);
        let line = value["line"].as_str().unwrap();
        assert!(!line.contains("secret text"), "got: {line}");
        assert!(line.contains(PLACEHOLDER));
    }

    #[test]
    fn redacts_home_paths_but_keeps_the_tail() {
        let out = redact(&patterns(), "opened /home/alice/.local/share/db");
        assert_eq!(out, "opened [REDACTED]/.local/share/db");
    }

    #[test]
    fn event_round_trip_redacts_the_message() {
        let event = Event {
            message: Some("token ".to_string() + &"f".repeat(40)),
            ..Default::default()
        };
        let redacted = redact_serialized(&patterns(), event).unwrap();
        assert_eq!(redacted.message.unwrap(), format!("token {PLACEHOLDER}"));
    }

    #[test]
    fn tail_drops_the_first_partial_line() {
        let tail = last_whole_lines("first line\nsecond line\nthird line\n", 20);
        assert!(!tail.contains("first"));
        assert!(tail.ends_with("third line\n"));
    }

    #[test]
    fn tail_never_cuts_into_a_line_that_does_not_fit() {
        let attachment = format!(r#"data: [{}]"#, "1, ".repeat(50));
        assert_eq!(last_whole_lines(&attachment, 20), "");
    }

    #[test]
    fn concat_separates_files_that_lack_a_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.log");
        let second = dir.path().join("b.log");
        std::fs::write(&first, "no trailing newline").unwrap();
        std::fs::write(&second, "next file\n").unwrap();

        let text = concat_files(&[first, second]).unwrap();
        assert_eq!(text, "no trailing newline\nnext file\n");
    }

    #[test]
    fn lists_only_log_files_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("old.log"), "a\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.path().join("new.log"), "b\n").unwrap();
        std::fs::write(dir.path().join("ignored.txt"), "c\n").unwrap();

        let files = list_log_files_oldest_first(dir.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["old.log", "new.log"]);
    }
}
