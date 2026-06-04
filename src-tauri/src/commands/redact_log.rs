use regex::Regex;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;
use tauri::{AppHandle, Manager};

use crate::filesystem::FileSystem;

const MAX_LOG_BYTES: usize = 5 * 1024 * 1024;

static REDACTION_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // FCM tokens — alphanumeric with colons, hyphens, underscores (100+ chars)
        r"[A-Za-z0-9_:\-]{100,}",
        // Hex strings (40+ chars) — public keys, hashes, signatures
        r"[0-9a-fA-F]{40,}",
        // Base64 blobs (40+ chars)
        r"[A-Za-z0-9+/]{40,}={0,2}",
        // DeviceId and AgentId wrappers (must precede bare PublicKey/Hash patterns)
        r"(DeviceId|AgentId)\([^)]*\([^)]*\)\)",
        // Debug-formatted byte arrays: PublicKey([1, 2, ...]), Hash([...]), Signature([...])
        r"(PublicKey|Hash|Signature)\(\[[\d, ]+\]\)",
        // Timestamps (seconds or microseconds since epoch, 10+ digits)
        r#""?timestamp"?\s*:?\s*\d{10,}"#,
        // Debug format: name/surname/about/description fields with quoted values
        r#"(name|surname|about|description):\s*(Some\()?"[^"]*"(\))?"#,
        // Debug format: ChatMessageContent("...") — legacy bare form, kept
        // in case rotating log buffers still contain entries from older builds.
        r#"ChatMessageContent\("[^"]*"\)"#,
        // Debug format: V0 unversioned content — ChatMessageContentV0("hello")
        r#"ChatMessageContentV0\("[^"]*"\)"#,
        // Debug format: V1 versioned content — `message: "hello"` inside
        // ChatMessageContentV1 { message: "...", media: ... }. Use \b so we
        // don't match substrings inside identifiers.
        r#"\bmessage:\s*"[^"]*""#,
        // Debug format: emoji: Some("...")
        r#"emoji:\s*Some\("[^"]*"\)"#,
        // JSON format: "name":"...", "surname":"...", "about":"...", "description":"..."
        r#""(name|surname|about|description)"\s*:\s*"[^"]*""#,
        // JSON format: "content":"..."
        r#""content"\s*:\s*"[^"]*""#,
        // JSON format: "emoji":"..."
        r#""emoji"\s*:\s*"[^"]*""#,
        // OS username inside filesystem paths. The whole `/home/<user>` (or
        // `/Users/<user>` / `\Users\<user>`) prefix is collapsed to [REDACTED];
        // the rest of the path is preserved so logs stay readable.
        r"/home/[^/\s]+",
        r"/Users/[^/\s]+",
        r"\\Users\\[^\\\s]+",
        // Hostname value (e.g. "Alices-MacBook-Pro.local" on macOS) — match
        // the whole `Hostname: <value>` line; both label and value are
        // identifying enough that we just drop the lot.
        r"Hostname:\s*[^\n\r]*",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid redaction pattern"))
    .collect()
});

/// Read `paths` in order, concatenate their contents, and tail the result
/// to at most `max_bytes`. When truncation happens, the first partial line
/// is dropped so the output starts on a clean log line.
fn read_concat_tail(paths: &[PathBuf], max_bytes: usize) -> std::io::Result<String> {
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

/// List every `*.log` file in `log_dir`, sorted by modification time
/// ascending (oldest first). `tauri-plugin-log` with KeepOne rotation
/// leaves a date-stamped older sibling alongside the live file; this
/// returns both so the caller can stitch the full history back together.
fn list_log_files_oldest_first(log_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(log_dir)?
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

pub fn redact(content: &str) -> String {
    let mut redacted = content.to_owned();
    for re in REDACTION_REGEXES.iter() {
        redacted = re.replace_all(&redacted, "[REDACTED]").into_owned();
    }
    redacted
}

#[tauri::command]
pub fn get_redacted_log(app_handle: AppHandle) -> Result<String, String> {
    let log_dir = FileSystem::new(&app_handle)
        .map_err(|e| format!("Failed to resolve log dir: {e:?}"))?
        .logs_dir();
    // tauri-plugin-log rotates `<package_name>.log` into a date-stamped
    // sibling under KeepOne, so the directory can hold one rotated file
    // plus the live one. Read both (oldest first) so the report covers
    // the full retained history, not just the post-rotation tail.
    let log_files = list_log_files_oldest_first(&log_dir)
        .map_err(|e| format!("Failed to list log dir {}: {e:?}", log_dir.display()))?;
    if log_files.is_empty() {
        return Err(format!("No *.log file in {}", log_dir.display()));
    }

    log::info!(
        "Redacting log files: {}",
        log_files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let content = read_concat_tail(&log_files, MAX_LOG_BYTES)
        .map_err(|e| format!("Failed to read logs in {}: {e:?}", log_dir.display()))?;

    let redacted = redact(&content);

    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve cache dir: {e:?}"))?;
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {e:?}"))?;

    let redacted_path = cache_dir.join("redacted-log.txt");
    std::fs::write(&redacted_path, redacted.as_bytes())
        .map_err(|e| format!("Failed to write redacted log: {e:?}"))?;

    Ok(redacted_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_hex_strings() {
        let input = "key=8d3ca6d66651182cd6a9c1fc5dad0260a0ee29fe9ed494734e60d259430ae8a4";
        assert_eq!(redact(input), "key=[REDACTED]");
    }

    #[test]
    fn preserves_short_hex() {
        let input = "code=abcdef12";
        assert_eq!(redact(input), "code=abcdef12");
    }

    #[test]
    fn redacts_base64_blobs() {
        let input = "data=SGVsbG8gV29ybGQgdGhpcyBpcyBhIGxvbmcgYmFzZTY0IHN0cmluZw==";
        assert_eq!(redact(input), "data=[REDACTED]");
    }

    #[test]
    fn redacts_public_key_byte_array() {
        let input = "got PublicKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]) from peer";
        assert_eq!(redact(input), "got [REDACTED] from peer");
    }

    #[test]
    fn redacts_hash_byte_array() {
        let input = "hash: Hash([177, 119, 236, 27, 242, 109, 251, 59, 112, 16, 212, 115, 230, 212, 71, 19, 178, 155, 118, 91, 153, 198, 230, 14, 203, 250, 231, 66, 222, 73, 101, 67])";
        assert_eq!(redact(input), "hash: [REDACTED]");
    }

    #[test]
    fn redacts_signature_byte_array() {
        let input = "sig: Signature([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64])";
        assert_eq!(redact(input), "sig: [REDACTED]");
    }

    #[test]
    fn redacts_device_id() {
        let input = "from DeviceId(PublicKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]))";
        assert_eq!(redact(input), "from [REDACTED]");
    }

    #[test]
    fn redacts_agent_id() {
        let input = "agent AgentId(ActorId([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]))";
        assert_eq!(redact(input), "agent [REDACTED]");
    }

    #[test]
    fn redacts_timestamps() {
        let input = "timestamp: 1708000000000000, other data";
        assert_eq!(redact(input), "[REDACTED], other data");
    }

    #[test]
    fn redacts_json_timestamp() {
        let input = r#""timestamp":1708000000000000"#;
        assert_eq!(redact(input), "[REDACTED]");
    }

    #[test]
    fn redacts_timestamp_in_seconds() {
        let input = "timestamp: 1708000000, other data";
        assert_eq!(redact(input), "[REDACTED], other data");
    }

    #[test]
    fn redacts_json_timestamp_in_seconds() {
        let input = r#""timestamp":1708000000"#;
        assert_eq!(redact(input), "[REDACTED]");
    }

    #[test]
    fn redacts_profile_name_debug() {
        let input =
            r#"Profile { name: "Alice", surname: Some("Smith"), about: Some("Hello world") }"#;
        let result = redact(input);
        assert!(!result.contains("Alice"), "name not redacted: {result}");
        assert!(!result.contains("Smith"), "surname not redacted: {result}");
        assert!(
            !result.contains("Hello world"),
            "about not redacted: {result}"
        );
    }

    #[test]
    fn redacts_profile_name_json() {
        let input = r#"{"name":"Alice","surname":"Smith","about":"Hello world"}"#;
        let result = redact(input);
        assert!(!result.contains("Alice"), "name not redacted: {result}");
        assert!(!result.contains("Smith"), "surname not redacted: {result}");
        assert!(
            !result.contains("Hello world"),
            "about not redacted: {result}"
        );
    }

    #[test]
    fn redacts_group_details_debug() {
        let input =
            r#"GroupDetails { name: "Family", description: Some("Secret plan"), image: None }"#;
        let result = redact(input);
        assert!(!result.contains("Family"), "name not redacted: {result}");
        assert!(
            !result.contains("Secret plan"),
            "description not redacted: {result}"
        );
    }

    #[test]
    fn redacts_group_details_json() {
        let input = r#"{"name":"Family","description":"Secret plan","image":null}"#;
        let result = redact(input);
        assert!(!result.contains("Family"), "name not redacted: {result}");
        assert!(
            !result.contains("Secret plan"),
            "description not redacted: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_debug() {
        let input = r#"ChatMessageContent("secret message here")"#;
        let result = redact(input);
        assert!(
            !result.contains("secret message"),
            "message not redacted: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_debug_v0_wrapped() {
        // Compat<…V0, …V> Debug for the V0 branch:
        let input = r#"ChatMessageContent(Unversioned(ChatMessageContentV0("secret v0 body")))"#;
        let result = redact(input);
        assert!(
            !result.contains("secret v0 body"),
            "v0 message not redacted: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_debug_v1_wrapped() {
        // Compat<…V0, …V> Debug for the V1 branch:
        let input = r#"ChatMessageContent(Versioned(V1(ChatMessageContentV1 { message: "secret v1 body", media: None })))"#;
        let result = redact(input);
        assert!(
            !result.contains("secret v1 body"),
            "v1 message not redacted: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_json() {
        let input = r#""content":"secret message here""#;
        let result = redact(input);
        assert!(
            !result.contains("secret message"),
            "message not redacted: {result}"
        );
    }

    #[test]
    fn redacts_reaction_debug() {
        let input = r#"emoji: Some("👍")"#;
        assert_eq!(redact(input), "[REDACTED]");
    }

    #[test]
    fn redacts_reaction_json() {
        let input = r#""emoji":"👍""#;
        assert_eq!(redact(input), "[REDACTED]");
    }

    #[test]
    fn redacts_hostname_line() {
        let input = "Hostname: Alices-MacBook-Pro.local";
        let result = redact(input);
        assert!(
            !result.contains("Alices"),
            "hostname not redacted: {result}"
        );
        assert!(
            !result.contains("MacBook-Pro"),
            "hostname not redacted: {result}"
        );
        assert_eq!(result, "[REDACTED]");
    }

    #[test]
    fn redacts_username_in_linux_path_keeps_rest() {
        let input = "App data dir: /home/alice/.local/share/dash-chat";
        let result = redact(input);
        assert!(!result.contains("alice"), "username not redacted: {result}");
        assert_eq!(result, "App data dir: [REDACTED]/.local/share/dash-chat",);
    }

    #[test]
    fn redacts_username_in_macos_path_keeps_rest() {
        let input = "App root dir: /Users/alice/Library/Application Support/dash-chat";
        let result = redact(input);
        assert!(!result.contains("alice"), "username not redacted: {result}");
        assert_eq!(
            result,
            "App root dir: [REDACTED]/Library/Application Support/dash-chat",
        );
    }

    #[test]
    fn redacts_username_in_windows_path_keeps_rest() {
        let input = "Logs dir: C:\\Users\\alice\\AppData\\Roaming\\dash-chat\\logs";
        let result = redact(input);
        assert!(!result.contains("alice"), "username not redacted: {result}");
        assert_eq!(
            result,
            "Logs dir: C:[REDACTED]\\AppData\\Roaming\\dash-chat\\logs",
        );
    }

    #[test]
    fn redacts_username_anywhere_paths_appear() {
        // Paths leak through many log lines, not just the device-info labels.
        let input = "Redacting log file: /home/alice/.local/share/dash-chat/logs/dash-chat.log";
        let result = redact(input);
        assert!(!result.contains("alice"), "username not redacted: {result}");
        assert!(
            result.contains("[REDACTED]/.local/share/dash-chat/logs/dash-chat.log"),
            "path tail should be preserved: {result}"
        );
    }

    #[test]
    fn preserves_non_sensitive_log_lines() {
        let input = "2024-02-15T10:30:00 INFO stream processing loop cancelled";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn read_concat_tail_small_single_file() {
        let dir = std::env::temp_dir().join("dashchat_test_concat_small");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("small.log");
        std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
        let result = read_concat_tail(&[path], 1024).unwrap();
        assert_eq!(result, "line1\nline2\nline3\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_concat_tail_truncates_large_single_file() {
        let dir = std::env::temp_dir().join("dashchat_test_concat_truncate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("large.log");
        let padding = "x".repeat(90) + "\n";
        let tail = "line_a\nline_b\nline_c\n";
        std::fs::write(&path, format!("{padding}{tail}")).unwrap();
        let result = read_concat_tail(&[path], 30).unwrap();
        assert!(
            !result.contains('x'),
            "padding should be truncated: {result}"
        );
        assert!(result.contains("line_b"), "should contain line_b: {result}");
        assert!(result.contains("line_c"), "should contain line_c: {result}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_concat_tail_joins_files_in_order() {
        let dir = std::env::temp_dir().join("dashchat_test_concat_join");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let older = dir.join("rotated.log");
        let newer = dir.join("live.log");
        std::fs::write(&older, "older1\nolder2\n").unwrap();
        std::fs::write(&newer, "newer1\nnewer2\n").unwrap();
        let result = read_concat_tail(&[older, newer], 1024).unwrap();
        assert_eq!(result, "older1\nolder2\nnewer1\nnewer2\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_concat_tail_inserts_newline_between_files() {
        let dir = std::env::temp_dir().join("dashchat_test_concat_newline");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.log");
        let b = dir.join("b.log");
        std::fs::write(&a, "tail_of_a").unwrap();
        std::fs::write(&b, "head_of_b\n").unwrap();
        let result = read_concat_tail(&[a, b], 1024).unwrap();
        assert_eq!(result, "tail_of_a\nhead_of_b\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn list_log_files_orders_by_mtime_ascending() {
        let dir = std::env::temp_dir().join("dashchat_test_list_logs");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let older = dir.join("older.log");
        let newer = dir.join("newer.log");
        let ignored = dir.join("notes.txt");
        std::fs::write(&older, "x\n").unwrap();
        std::fs::write(&ignored, "ignore\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&newer, "y\n").unwrap();

        let result = list_log_files_oldest_first(&dir).unwrap();
        assert_eq!(result, vec![older, newer]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn redacts_fcm_token() {
        let input = "New FCM token: dOBkZ7QjS_eSLaFMw3-LbX:APA91bH0P1NpdH4BxdK3YnE7xA3TN4k-example-token-that-is-very-long-and-contains-colons-hyphens-underscores";
        let result = redact(input);
        assert!(
            !result.contains("dOBkZ7QjS_eSLaFMw3"),
            "FCM token not redacted: {result}"
        );
    }

    #[test]
    fn redacts_full_notification_log_line() {
        let input = r#"2024-02-15 INFO Received notification: Chat(Message(ChatMessageContent("hey there"))) from DeviceId(PublicKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]))"#;
        let result = redact(input);
        assert!(
            !result.contains("hey there"),
            "message not redacted: {result}"
        );
        assert!(!result.contains("32, 145"), "key not redacted: {result}");
    }
}
