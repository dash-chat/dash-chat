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
        // iroh/p2panda node ids in short hex form (`me=fa30b1af97`,
        // `peer=0a753b78eb`) that the 40-char hex rule above is too long to
        // catch. Anchored on the `me=`/`peer=` label so ordinary short hex
        // (contact codes) stays readable.
        r"\b(me|peer)=[0-9a-fA-F]{8,}\b",
        // The ids p2panda-net's discovery, gossip and address book label, which
        // the rule above is too long to catch: sampling a run's Debug output
        // shows `endpoint_id=…`, `node_id="…"`, `remote_node_id=…` and the
        // topic ones — `topic=…`, `gossip_topic="…"`, `sync_topic="…"`, which
        // name the conversation a device is in. Any prefix, an `_id` suffix or
        // a plural, and either separator; the quotes are consumed so nothing
        // survives as `""`. `alpn=`/`protocol_id=` are left: they are the same
        // constant for every user of a build, and say nothing about who is
        // using it.
        r#"\b[a-z_]*(node_id|endpoint_id|topic(_id)?)s?["\s]*[=:]\s*"?[0-9a-fA-F]{8,}"?"#,
        // The peers a gossip overlay joins, which the rule above misses on both
        // counts — the label is `nodes`, not `node_id`, and the ids sit inside
        // brackets. Sampling one run's os_log output found this shape 856 times
        // (`(re-) join gossip overlay topic=… nodes=[…]`, `joined topic …`),
        // naming who a device is gossiping with. The list is taken whole, so a
        // second id cannot survive by being unlabelled. The `_id` and endpoint
        // spellings are covered too, for a version that labels the same list
        // differently: what the sample shows is the 10-character short form, so
        // the 40-char hex rule above is no backstop for any of them. An empty
        // `nodes=[]` stays readable: it says nobody was found, which is what a
        // discovery failure looks like, and it names no one.
        r"\b[a-z_]*(node|endpoint)(_id)?s?=\[[0-9a-fA-F][0-9a-fA-F,\s]*\]",
        // Socket addresses of peers and of this device, as the address book
        // prints them: `Ip(188.84.6.11:49882)`, and bracketed for v6,
        // `Ip([2a02:…:1]:41234)` / `Ip([fe80::…%en0]:…)`. A peer's address says
        // who a user is talking to and the public one says where they are.
        // Anchored on p2panda's `Ip(…)` wrapper so the urls the app logs —
        // `MAILBOX_URL: http://192.168.0.104:4338`, and the ones a connection
        // error names — stay readable, since a wrong one is only ever spotted
        // by reading it back.
        r"Ip\(\[?[0-9a-zA-Z:.%_-]+\]?:[0-9]{1,5}\)",
        // Base64 blobs (40+ chars)
        r"[A-Za-z0-9+/]{40,}={0,2}",
        // Mailbox id (base64url inbox address) as logged by the mailbox
        // manager: `polling mailbox <id>` / `mailbox=<id>`. The base64 rule
        // above misses it because the url-safe `-`/`_` split it below 40
        // unbroken chars. Anchored on the label so it can't over-match other
        // long url-safe tokens.
        r"\bmailbox[ =:]+[A-Za-z0-9_\-]{20,}",
        // Device / app-group container UUID, which only appears as a path
        // segment (`<app_root>/<UUID>/0.13`). Anchored on the trailing `/`
        // (the regex crate has no lookahead, so the slash is consumed) so it
        // can't match bare UUID leaves such as Sentry's `debug_id` — redacting
        // those makes the event fail to deserialize back into a DebugId and the
        // whole report is dropped as unredactable.
        r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}/",
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
        // Debug format: NotificationData fields carrying user content — title
        // (sender or group name), body/large_body/summary (message text), and
        // conversation_title (group name). The NSE logs the built notification,
        // so these reach a report attachment and must be stripped.
        r#"\b(title|body|large_body|summary|conversation_title):\s*(Some\()?"[^"]*"(\))?"#,
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
        // JSON format: notification title/body/summary/conversation_title.
        r#""(title|body|large_body|summary|conversation_title)"\s*:\s*"[^"]*""#,
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
    fn redacts_discovery_topic_ids() {
        let input = "(re-) join gossip overlay topic=371ac34c42 nodes=[]";
        assert_eq!(
            redact(input),
            "(re-) join gossip overlay [REDACTED] nodes=[]"
        );
        let input = "register sync protocol sync_topic=\"02d9de2757\"";
        assert_eq!(redact(input), "register sync protocol [REDACTED]");
        // The `_id` suffix, which nothing logs today — covered before
        // something starts to.
        let input = "subscribing topic_id=371ac34c42";
        assert_eq!(redact(input), "subscribing [REDACTED]");
    }

    /// The exact lines one run's os_log produced, as the gossip and discovery
    /// modules write them at Debug.
    #[test]
    fn redacts_the_peers_a_gossip_overlay_names() {
        let input = "(re-) join gossip overlay topic=d63c2396b0 nodes=[9b26ccaba4]";
        assert_eq!(
            redact(input),
            "(re-) join gossip overlay [REDACTED] [REDACTED]"
        );
        let input = "joined topic topic=0083bbda68 nodes=[a64c9c1b7f, 7dd414f859]";
        assert_eq!(redact(input), "joined topic [REDACTED] [REDACTED]");
        // Finding nobody is not private, and it is what a discovery failure
        // looks like.
        let input = "(re-) join gossip overlay topic=d63c2396b0 nodes=[]";
        assert_eq!(
            redact(input),
            "(re-) join gossip overlay [REDACTED] nodes=[]"
        );
        // The spellings the sample did not happen to show. What p2panda logs is
        // the 10-character short form, so nothing else would catch these.
        let input = "peers node_ids=[a64c9c1b7f, 7dd414f859] endpoint_ids=[9b26ccaba4]";
        assert_eq!(redact(input), "peers [REDACTED] [REDACTED]");
    }

    #[test]
    fn preserves_the_networks_own_constants() {
        // The same for every user of a build, so they name nobody.
        let input = "register protocol alpn=d129148097";
        assert_eq!(redact(input), "register protocol alpn=d129148097");
    }

    #[test]
    fn redacts_discovery_node_ids() {
        let input = "mark node as stale remote_node_id=3026d92c8c";
        assert_eq!(redact(input), "mark node as stale [REDACTED]");
        let input = "successful discovery session node_id=\"5299f918f8\" topics=5";
        assert_eq!(
            redact(input),
            "successful discovery session [REDACTED] topics=5"
        );
        let input = "discovered new transport info endpoint_id=fa09a0a99a";
        assert_eq!(redact(input), "discovered new transport info [REDACTED]");
    }

    #[test]
    fn redacts_peer_socket_addresses() {
        let input = "addresses=[iroh] {Ip(188.84.6.11:49882), Ip(192.168.0.106:65133)}";
        assert_eq!(redact(input), "addresses=[iroh] {[REDACTED], [REDACTED]}");
    }

    #[test]
    fn redacts_ipv6_peer_socket_addresses() {
        let input =
            "addresses=[iroh] {Ip([2a02:8109:a1c0::1]:41234), Ip([fe80::1ff:fe23:4567%en0]:5353)}";
        assert_eq!(redact(input), "addresses=[iroh] {[REDACTED], [REDACTED]}");
    }

    #[test]
    fn preserves_the_url_a_build_is_pointed_at() {
        // Every url the app logs carries a port, and reading one back is how a
        // build pointed at the wrong mailbox gets spotted.
        let input = "Using compile-time MAILBOX_URL: http://192.168.0.104:4338";
        assert_eq!(
            redact(input),
            "Using compile-time MAILBOX_URL: http://192.168.0.104:4338"
        );
        let input = "error sending request for url (http://127.0.0.1:3200/health)";
        assert_eq!(
            redact(input),
            "error sending request for url (http://127.0.0.1:3200/health)"
        );
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
        // The waveform is lossy downsampled amplitude, not recoverable audio, so
        // only the bytes are stripped.
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
    fn redacts_notification_data_debug() {
        // The shape the NSE logs when showing a built notification: sender name
        // in `title`, message text in `body`, group name in `conversation_title`.
        let input = r#"NotificationData { id: 1, title: Some("Macky"), body: Some("lalala"), large_body: None, summary: None, conversation_style: Some(ConversationStyle { sender_id: Some("abc"), conversation_title: Some("Family Chat") }) }"#;
        let result = redact(input);
        assert!(!result.contains("Macky"), "title (name) leaked: {result}");
        assert!(
            !result.contains("lalala"),
            "body (message) leaked: {result}"
        );
        assert!(
            !result.contains("Family Chat"),
            "conversation_title leaked: {result}"
        );
    }

    #[test]
    fn redacts_notification_data_json() {
        let input = r#"{"title":"Macky","body":"lalala","large_body":"long text","summary":"2 messages","conversation_title":"Family Chat"}"#;
        let result = redact(input);
        assert!(!result.contains("Macky"), "title leaked: {result}");
        assert!(!result.contains("lalala"), "body leaked: {result}");
        assert!(!result.contains("long text"), "large_body leaked: {result}");
        assert!(!result.contains("2 messages"), "summary leaked: {result}");
        assert!(
            !result.contains("Family Chat"),
            "conversation_title leaked: {result}"
        );
    }

    #[test]
    fn preserves_large_body_field_name_boundary() {
        // `\b` must not let the `body` alternative match inside `large_body`;
        // `large_body` is covered by its own alternative, but a bare
        // `large_body: None` (no quoted value) must be left untouched.
        let input = r#"large_body: None, body: Some("hi")"#;
        let result = redact(input);
        assert!(
            result.contains("large_body: None"),
            "over-redacted: {result}"
        );
        assert!(!result.contains("hi"), "body not redacted: {result}");
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

    #[test]
    fn redacts_short_form_node_keys() {
        let input = "gossip; me=fa30b1af97 conn; peer=0a753b78eb";
        let result = redact(input);
        assert!(!result.contains("fa30b1af97"), "me key leaked: {result}");
        assert!(!result.contains("0a753b78eb"), "peer key leaked: {result}");
    }

    #[test]
    fn preserves_short_hex_contact_code() {
        // The short-form key rule must stay anchored to `me=`/`peer=` and not
        // touch other short hex like contact codes.
        let input = "code=abcdef12";
        assert_eq!(redact(input), "code=abcdef12");
    }

    #[test]
    fn redacts_mailbox_id() {
        let input = "polling mailbox 2wgdUYYgohPKdhkjbgmBjlZgfh-hZBHVpi6GkXkRYxc";
        let result = redact(input);
        assert!(
            !result.contains("2wgdUYYgohPKdhkjbgmBjlZgfh"),
            "mailbox id leaked: {result}"
        );
    }

    #[test]
    fn redacts_mailbox_id_key_value_form() {
        let input = "mailbox=2wgdUYYgohPKdhkjbgmBjlZgfh-hZBHVpi6GkXkRYxc sync error";
        let result = redact(input);
        assert!(
            !result.contains("2wgdUYYgohPKdhkjbgmBjlZgfh"),
            "mailbox id leaked: {result}"
        );
    }

    #[test]
    fn preserves_mailbox_module_path() {
        let input = "mailbox_client::manager crates/mailbox-client/src/manager.rs:719";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn redacts_device_container_uuid() {
        let input = "data path: 1A2B3C4D-F4CD-4E51-B851-69CF2F22D0AA/0.13";
        let result = redact(input);
        assert!(
            !result.contains("1A2B3C4D-F4CD-4E51-B851-69CF2F22D0AA"),
            "uuid leaked: {result}"
        );
    }

    #[test]
    fn preserves_bare_uuid_debug_id() {
        // Sentry's `debug_id` is a bare UUID leaf that must stay parseable when
        // the event is re-serialized; the container-UUID rule must only fire in
        // path context, never on a standalone UUID.
        let input = "84a04d24-0e60-3810-a8c0-90d5b4f8e4a3";
        assert_eq!(redact(input), input);
    }
}
