use regex::Regex;
use std::io::{Read, Seek, SeekFrom};
use std::sync::LazyLock;
use tauri::{AppHandle, Manager};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

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
        // Debug format: name/surname/about fields with quoted values
        r#"(name|surname|about):\s*(Some\()?"[^"]*"(\))?"#,
        // Debug format: ChatMessageContent("...")
        r#"ChatMessageContent\("[^"]*"\)"#,
        // Debug format: emoji: Some("...")
        r#"emoji:\s*Some\("[^"]*"\)"#,
        // JSON format: "name":"...", "surname":"...", "about":"..."
        r#""(name|surname|about)"\s*:\s*"[^"]*""#,
        // JSON format: "content":"..."
        r#""content"\s*:\s*"[^"]*""#,
        // JSON format: "emoji":"..."
        r#""emoji"\s*:\s*"[^"]*""#,
    ]
    .iter()
    .map(|p| Regex::new(p).expect("invalid redaction pattern"))
    .collect()
});

fn read_tail(path: &std::path::Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        file.seek(SeekFrom::End(-(max_bytes as i64)))?;
    }
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    if len > max_bytes {
        // Drop the first partial line
        if let Some(pos) = buf.find('\n') {
            buf.drain(..=pos);
        }
    }
    Ok(buf)
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
    let log_dir = app_handle
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log dir: {e:?}"))?;
    let log_file = log_dir.join("Dash Chat.log");

    let content =
        read_tail(&log_file, MAX_LOG_BYTES).map_err(|e| format!("Failed to read log: {e:?}"))?;

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
    fn redacts_chat_message_debug() {
        let input = r#"ChatMessageContent("secret message here")"#;
        let result = redact(input);
        assert!(
            !result.contains("secret message"),
            "message not redacted: {result}"
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
    fn preserves_non_sensitive_log_lines() {
        let input = "2024-02-15T10:30:00 INFO stream processing loop cancelled";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn read_tail_small_file() {
        let dir = std::env::temp_dir().join("dashchat_test_read_tail_small");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("small.log");
        std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
        let result = read_tail(&path, 1024).unwrap();
        assert_eq!(result, "line1\nline2\nline3\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_tail_truncates_large_file() {
        let dir = std::env::temp_dir().join("dashchat_test_read_tail_large");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("large.log");
        // Write 100 bytes of padding then 3 meaningful lines
        let padding = "x".repeat(90) + "\n";
        let tail = "line_a\nline_b\nline_c\n";
        std::fs::write(&path, format!("{padding}{tail}")).unwrap();
        // Read only last 30 bytes — should drop the first partial line
        let result = read_tail(&path, 30).unwrap();
        assert!(
            !result.contains('x'),
            "padding should be truncated: {result}"
        );
        assert!(result.contains("line_b"), "should contain line_b: {result}");
        assert!(result.contains("line_c"), "should contain line_c: {result}");
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
