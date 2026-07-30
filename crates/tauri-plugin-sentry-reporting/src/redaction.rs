use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use sentry::protocol::Event;
use serde_json::Value;

const PLACEHOLDER: &str = "[REDACTED]";

/// Public so the app can assert its patterns against the real implementation
/// rather than a copy of it.
pub fn redact(patterns: &[Regex], text: &str) -> String {
    let mut out = text.to_owned();
    for re in patterns {
        out = re.replace_all(&out, PLACEHOLDER).into_owned();
    }
    out
}

/// Per leaf, not over the serialized document: several patterns match a whole
/// `"key": "value"` pair and would replace it wholesale, producing invalid JSON.
/// Per leaf they still fire against string contents, including JSON embedded in
/// a log line as text.
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

/// Round-trips through JSON so every string the event carries is covered.
pub(crate) fn redact_event(
    patterns: &[Regex],
    event: Event<'static>,
) -> anyhow::Result<Event<'static>> {
    let mut value = serde_json::to_value(event)?;
    redact_json_leaves(patterns, &mut value);
    Ok(serde_json::from_value(value)?)
}

/// Tails to `max_bytes`, dropping the leading partial line.
pub(crate) fn read_concat_tail(paths: &[PathBuf], max_bytes: usize) -> std::io::Result<String> {
    let mut buf = String::new();
    for path in paths {
        let mut file = std::fs::File::open(path)?;
        file.read_to_string(&mut buf)?;
        if !buf.is_empty() && !buf.ends_with('\n') {
            buf.push('\n');
        }
    }
    if buf.len() > max_bytes {
        let mut start = buf.len() - max_bytes;
        while start < buf.len() && !buf.is_char_boundary(start) {
            start += 1;
        }
        buf.drain(..start);
        if let Some(pos) = buf.find('\n') {
            buf.drain(..=pos);
        }
    }
    Ok(buf)
}

/// Oldest first. KeepOne rotation leaves a date-stamped sibling next to the live
/// file, and the report wants both.
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

/// Public so the app's debug-log export shares this one implementation.
pub fn redacted_log_tail(
    patterns: &[Regex],
    logs_dir: &Path,
    max_bytes: usize,
) -> anyhow::Result<String> {
    let files = list_log_files_oldest_first(logs_dir)?;
    if files.is_empty() {
        anyhow::bail!("no log files in {}", logs_dir.display());
    }
    Ok(redact(patterns, &read_concat_tail(&files, max_bytes)?))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn leaf_redaction_keeps_the_document_parseable() {
        let mut value = serde_json::json!({ "message": r#"{"message":"x"}"# });
        redact_json_leaves(&patterns(), &mut value);
        let text = serde_json::to_string(&value).unwrap();
        serde_json::from_str::<Value>(&text).expect("still valid JSON");
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
        let redacted = redact_event(&patterns(), event).unwrap();
        assert_eq!(redacted.message.unwrap(), format!("token {PLACEHOLDER}"));
    }

    #[test]
    fn tail_drops_the_first_partial_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.log");
        std::fs::write(&path, "first line\nsecond line\nthird line\n").unwrap();

        let tail = read_concat_tail(&[path], 20).unwrap();
        assert!(!tail.contains("first"));
        assert!(tail.ends_with("third line\n"));
    }

    #[test]
    fn tail_returns_everything_when_under_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.log");
        std::fs::write(&path, "only line\n").unwrap();
        assert_eq!(read_concat_tail(&[path], 4096).unwrap(), "only line\n");
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
