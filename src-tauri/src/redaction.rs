use regex::Regex;
use std::sync::LazyLock;

/// What counts as sensitive. `tauri-plugin-sentry-reporting` applies these to
/// everything on its way off the device, so any feature carrying private or
/// user-generated data needs a pattern here.
pub static REDACTION_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // FCM tokens — alphanumeric with colons, hyphens, underscores (100+ chars)
        r"[A-Za-z0-9_:\-]{100,}",
        // Hex strings (40+ chars) — public keys, hashes, signatures
        r"[0-9a-fA-F]{40,}",
        // Base64 blobs (40+ chars)
        r"[A-Za-z0-9+/]{40,}={0,2}",
        // DeviceId and AgentId wrappers (must precede bare VerifyingKey/Hash patterns)
        r"(DeviceId|AgentId)\([^)]*\([^)]*\)\)",
        // Debug-formatted byte arrays: VerifyingKey([1, 2, ...]), Hash([...]), Signature([...]), InboxNonce([...])
        r"(VerifyingKey|Hash|Signature|InboxNonce)\(\[[\d, ]+\]\)",
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
        // Media attachment byte arrays in Debug (`data: [1, 2, ...]`) and
        // JSON (`"data":[1,2,...]`) form, inside Photo/FileAttachment.
        // Strips the bytes so attachment content never leaves the device in
        // a log report. Attachment filenames need no patterns of their own:
        // the Debug `name:` and JSON "name" patterns above already cover
        // them.
        r#"\bdata:\s*\[[\d,\s]*\]"#,
        r#""data"\s*:\s*\[[\d,\s]*\]"#,
        // Debug format: emoji: Some("...")
        r#"emoji:\s*Some\("[^"]*"\)"#,
        // JSON format: "name":"...", "surname":"...", "about":"...", "description":"..."
        r#""(name|surname|about|description)"\s*:\s*"[^"]*""#,
        // JSON format: "profile_name":"..." — contact request QR placeholder.
        r#""profile_name"\s*:\s*"[^"]*""#,
        // JSON format: "content":"..."
        r#""content"\s*:\s*"[^"]*""#,
        // JSON format: "message":"..." — chat message text and edit text
        // (ChatMessageContentV1 and EditMessage both serialize a `message` field).
        r#""message"\s*:\s*"[^"]*""#,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert the patterns through the same function that runs at egress.
    fn redact(input: &str) -> String {
        tauri_plugin_sentry_reporting::redact(&REDACTION_REGEXES, input)
    }

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
    fn redacts_verifying_key_byte_array() {
        let input = "got VerifyingKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]) from peer";
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
        let input = "from DeviceId(VerifyingKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]))";
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
    fn redacts_profile_name_field_debug() {
        let input = r#"PendingContactRequest { device_pubkey: VerifyingKey([1, 2, 3]), profile_name: "Alice" }"#;
        let result = redact(input);
        assert!(
            !result.contains("Alice"),
            "profile_name not redacted: {result}"
        );
    }

    #[test]
    fn redacts_profile_name_field_json() {
        let input = r#"{"type":"PendingContactRequest","payload":{"device_pubkey":[1,2,3],"profile_name":"Alice"}}"#;
        let result = redact(input);
        assert!(
            !result.contains("Alice"),
            "profile_name not redacted: {result}"
        );
    }

    #[test]
    fn redacts_group_info_debug() {
        let input =
            r#"GroupInfo { name: "Family", description: Some("Secret plan"), image: None }"#;
        let result = redact(input);
        assert!(!result.contains("Family"), "name not redacted: {result}");
        assert!(
            !result.contains("Secret plan"),
            "description not redacted: {result}"
        );
    }

    #[test]
    fn redacts_group_info_json() {
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
    fn redacts_chat_message_media_photo() {
        let input = r#"ChatMessageContentV1 { message: "caption", media: Some(Photos { photos: [Photo { data: [137, 80, 78, 71, 13, 10, 26, 10], name: "private.jpg", mime_type: "image/jpeg" }] }) }"#;
        let result = redact(input);
        assert!(!result.contains("caption"), "caption leaked: {result}");
        assert!(
            !result.contains("137, 80, 78"),
            "photo bytes leaked: {result}"
        );
        assert!(
            !result.contains("private.jpg"),
            "photo filename leaked: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_media_file() {
        let input = r#"ChatMessageContentV1 { message: "", media: Some(File { file: FileAttachment { data: [1, 2, 3, 4, 5], name: "secrets.pdf", mime_type: "application/pdf" } }) }"#;
        let result = redact(input);
        assert!(
            !result.contains("1, 2, 3, 4, 5"),
            "file bytes leaked: {result}"
        );
        assert!(
            !result.contains("secrets.pdf"),
            "file name leaked: {result}"
        );
    }

    #[test]
    fn redacts_chat_message_media_voice() {
        let voice = dashchat_node::OutgoingMedia::VoiceNote {
            voice_note: dashchat_node::OutgoingVoiceNote {
                data: vec![255, 251, 144, 0, 7, 8],
                mime_type: "audio/wav".into(),
                duration_ms: 4200,
                waveform: vec![0, 128, 255],
            },
        };
        let input = format!("{voice:?}");
        let result = redact(&input);
        // The recorded audio bytes are private and must be stripped. The
        // waveform is lossy downsampled amplitude (not recoverable audio), so
        // it is left readable for debugging.
        assert!(
            !result.contains("255, 251, 144"),
            "voice bytes leaked: {result}"
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
    fn redacts_media_bytes_json() {
        let input = r#"{"photos":[{"data":[137, 80, 78, 71],"name":"private.jpg","mime_type":"image/jpeg"}]}"#;
        let result = redact(input);
        assert!(
            !result.contains("137, 80, 78, 71"),
            "photo bytes leaked: {result}"
        );
        assert!(
            !result.contains("private.jpg"),
            "photo name leaked: {result}"
        );
    }

    #[test]
    fn redacts_edit_message_debug() {
        let input = r#"Chat(EditMessage { message: "edited secret", edit_hash: Hash([1, 2, 3]) })"#;
        let result = redact(input);
        assert!(
            !result.contains("edited secret"),
            "edit text not redacted: {result}"
        );
    }

    #[test]
    fn redacts_edit_message_json() {
        let input =
            r#"{"type":"EditMessage","payload":{"message":"edited secret","edit_hash":"abc"}}"#;
        let result = redact(input);
        assert!(
            !result.contains("edited secret"),
            "edit text not redacted: {result}"
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
        let input = r#"2024-02-15 INFO Received notification: Chat(Message(ChatMessageContent("hey there"))) from DeviceId(VerifyingKey([32, 145, 78, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]))"#;
        let result = redact(input);
        assert!(
            !result.contains("hey there"),
            "message not redacted: {result}"
        );
        assert!(!result.contains("32, 145"), "key not redacted: {result}");
    }
}
