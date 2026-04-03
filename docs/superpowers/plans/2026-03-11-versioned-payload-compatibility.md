# Versioned Payload Compatibility Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a generic versioning system for p2panda payload types, starting with `ChatMessageContent`, so that newer and older clients can interoperate.

**Architecture:** A generic `Compat<Bare, Tagged>` enum with custom serde handles V0 (bare, unversioned) vs V1+ (internally tagged) wire formats. A `Capability` enum + `Capabilities` map tracks what each peer supports. A `VersionConvert` trait handles conversion between versions. Version info is exchanged via contact codes and announcements.

**Tech Stack:** Rust, serde, CBOR (via p2panda_core::cbor), p2panda

**Spec:** `docs/superpowers/specs/2026-03-11-versioned-payload-compatibility-design.md`

---

## File Structure

### New files
- `crates/dashchat-node/src/compat.rs` — `Compat<Bare, Tagged>` enum, custom Serialize/Deserialize, `VersionConvert` trait, `VersionConvertError`, `Capability` enum, `Capabilities` type alias
- `crates/dashchat-node/src/compat_tests.rs` — Unit tests for `Compat` serde round-trips with CBOR

### Modified files
- `crates/dashchat-node/src/lib.rs` — Add `mod compat; pub use compat::*;`
- `crates/dashchat-node/src/chat/message.rs` — Refactor `ChatMessageContent` into versioned form using `Compat`
- `crates/dashchat-node/src/chat/message.rs` (testing module) — Update `ChatMessage` to use the versioned type's getters
- `crates/dashchat-node/src/payload.rs` — Add `SetCapabilities` variant to `AnnouncementsPayload`
- `crates/dashchat-node/src/contact.rs` — Add `capabilities` field to `QrCode`
- `crates/dashchat-node/src/node.rs` — Add capability lookup + version conversion in `send_message`
- `src-tauri/src/commands/direct_chats.rs` — Minor: adapt to new `ChatMessageContent` type if needed
- `src-tauri/src/commands/redact_log.rs` — Update redaction patterns/tests for new debug format

---

## Chunk 1: Core Compat Infrastructure

### Task 1: Create `Compat<Bare, Tagged>` with custom serde

**Files:**
- Create: `crates/dashchat-node/src/compat.rs`
- Create: `crates/dashchat-node/src/compat_tests.rs`
- Modify: `crates/dashchat-node/src/lib.rs:1-11` (module declarations)

- [ ] **Step 1: Write failing test — Compat round-trip with CBOR for bare V0 type**

Create `crates/dashchat-node/src/compat_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use p2panda_core::cbor::{decode_cbor, encode_cbor};
    use serde::{Deserialize, Serialize};

    use comcap::Compat;

    /// A simple bare V0 type for testing.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct BareString(String);

    /// A tagged V1+ type for testing.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "v")]
    enum TestVersions {
        #[serde(rename = "1")]
        V1(TestV1),
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestV1 {
        message: String,
        extra: u32,
    }

    type TestCompat = Compat<BareString, TestVersions>;

    #[test]
    fn compat_roundtrip_v0() {
        let v0 = TestCompat::Unversioned(BareString("hello".into()));
        let bytes = encode_cbor(&v0).unwrap();

        // V0 should serialize identically to the bare type
        let bare_bytes = encode_cbor(&BareString("hello".into())).unwrap();
        assert_eq!(bytes, bare_bytes, "V0 must serialize as bare type");

        // Should round-trip back
        let decoded: TestCompat = decode_cbor(&bytes).unwrap();
        assert_eq!(decoded, v0);
    }

    #[test]
    fn compat_roundtrip_v1() {
        let v1 = TestCompat::Versioned(TestVersions::V1(TestV1 {
            message: "hello".into(),
            extra: 42,
        }));
        let bytes = encode_cbor(&v1).unwrap();

        // Should round-trip back
        let decoded: TestCompat = decode_cbor(&bytes).unwrap();
        assert_eq!(decoded, v1);
    }

    #[test]
    fn compat_deserialize_bare_bytes_as_v0() {
        // Bytes encoded by an old client as just a BareString
        let bare_bytes = encode_cbor(&BareString("from old client".into())).unwrap();
        let decoded: TestCompat = decode_cbor(&bare_bytes).unwrap();
        assert_eq!(
            decoded,
            TestCompat::Unversioned(BareString("from old client".into()))
        );
    }

    #[test]
    fn compat_deserialize_tagged_bytes_as_v1() {
        // Bytes encoded with the version tag
        let tagged_bytes = encode_cbor(&TestVersions::V1(TestV1 {
            message: "from new client".into(),
            extra: 99,
        }))
        .unwrap();
        let decoded: TestCompat = decode_cbor(&tagged_bytes).unwrap();
        assert_eq!(
            decoded,
            TestCompat::Versioned(TestVersions::V1(TestV1 {
                message: "from new client".into(),
                extra: 99,
            }))
        );
    }

    #[test]
    fn compat_unknown_version_fails() {
        // Simulate a V2 message (unknown to this client)
        // It's a map with "v": "2" — should fail both Tagged and Bare deser
        let unknown_bytes = encode_cbor(&serde_json::json!({
            "v": "2",
            "message": "future data",
            "new_field": true
        }))
        .unwrap();
        let result = decode_cbor::<TestCompat>(&unknown_bytes);
        assert!(result.is_err(), "Unknown version should fail to deserialize");
    }
}
```

- [ ] **Step 2: Write the `Compat` type with custom Serialize/Deserialize**

Create `crates/dashchat-node/src/compat.rs`.

The deserializer uses `#[serde(untagged)]` on an internal helper enum to try Tagged first, then Bare. This is format-agnostic (works with CBOR from p2panda AND JSON from Tauri commands). No `ciborium` dependency needed.

```rust
use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Transparent versioning wrapper.
///
/// - `Bare`: the V0 type (serialized without any wrapper, identical to pre-versioning format)
/// - `Tagged`: an enum of V1+ versions (internally tagged with `"v"` field via `#[serde(tag = "v")]`)
///
/// **Deserialization**: tries `Tagged` first (expects map with `"v"` key), falls back to `Bare`.
/// Uses `#[serde(untagged)]` internally, so works with any serde format (CBOR, JSON, etc.).
///
/// **Constraint**: `Bare` must not serialize as a map with a `"v"` key at the top level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compat<Bare, Tagged> {
    Unversioned(Bare),
    Versioned(Tagged),
}

impl<Bare, Tagged> Serialize for Compat<Bare, Tagged>
where
    Bare: Serialize,
    Tagged: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Compat::Unversioned(bare) => bare.serialize(serializer),
            Compat::Versioned(tagged) => tagged.serialize(serializer),
        }
    }
}

impl<'de, Bare, Tagged> Deserialize<'de> for Compat<Bare, Tagged>
where
    Bare: Deserialize<'de>,
    Tagged: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // serde's untagged deserialization: tries variants in declaration order.
        // Tagged is first — it requires a "v" key, so it fails fast on Bare data.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<B, T> {
            Tagged(T),
            Bare(B),
        }

        match Helper::<Bare, Tagged>::deserialize(deserializer)? {
            Helper::Tagged(t) => Ok(Compat::Versioned(t)),
            Helper::Bare(b) => Ok(Compat::Unversioned(b)),
        }
    }
}

/// A named feature domain that groups related versioned types.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    Messaging,
}

/// Maps each capability to the highest version the client supports.
/// Absence of a capability implies V0.
pub type Capabilities = BTreeMap<Capability, u16>;

/// Error returned by `VersionConvert::to_version`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionConvertError {
    /// The content can't be represented at the target version without
    /// unacceptable data loss (e.g., media-only message → V0).
    Lossy,
    /// The target version is not recognized.
    UnknownVersion,
}

impl fmt::Display for VersionConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionConvertError::Lossy => write!(f, "lossy version conversion"),
            VersionConvertError::UnknownVersion => write!(f, "unknown target version"),
        }
    }
}

impl std::error::Error for VersionConvertError {}

/// Trait for types that can be converted between versions.
///
/// Implement this for each `Compat<V0, Versions>` type.
/// The compiler enforces exhaustive match across all version variants.
pub trait VersionConvert: Sized {
    /// Which capability governs this type's versioning.
    const CAPABILITY: Capability;

    /// Convert self to a representation at the target version.
    /// Returns `Err(Lossy)` if the content can't be meaningfully represented.
    /// Returns `Err(UnknownVersion)` if the target version is unrecognized.
    fn to_version(&self, target_version: u16) -> Result<Self, VersionConvertError>;
}

#[cfg(test)]
#[path = "compat_tests.rs"]
mod compat_tests;
```

- [ ] **Step 3: Register the module in lib.rs**

In `crates/dashchat-node/src/lib.rs`, add `mod compat;` and `pub use compat::*;` alongside the existing module declarations.

Add after line 5 (`mod error;`):
```rust
mod compat;
```

Add after line 31 (`pub use payload::*;`):
```rust
pub use compat::*;
```

- [ ] **Step 4: Run tests to verify round-trips**

Run: `cargo test -p dashchat-node compat_tests`

Expected: All 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/dashchat-node/src/compat.rs crates/dashchat-node/src/compat_tests.rs crates/dashchat-node/src/lib.rs
git commit -m "feat: add Compat<Bare, Tagged> versioning wrapper with serde + CBOR support"
```

---

## Chunk 2: Versioned ChatMessageContent

### Task 2: Refactor ChatMessageContent to use Compat

**Files:**
- Modify: `crates/dashchat-node/src/chat/message.rs:1-112`
- Modify: `crates/dashchat-node/src/payload.rs:11` (import)
- Modify: `crates/dashchat-node/src/node.rs:29` (import)

- [ ] **Step 1: Write failing test — ChatMessageContent V0 round-trip**

Add to `crates/dashchat-node/src/compat_tests.rs` (new test module section):

```rust
#[cfg(test)]
mod chat_message_compat_tests {
    use p2panda_core::cbor::{decode_cbor, encode_cbor};

    use crate::chat::{ChatMessageContent, ChatMessageContentV0};
    use comcap::Compat;

    #[test]
    fn chat_message_v0_roundtrip() {
        let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
        let bytes = encode_cbor(&v0).unwrap();

        // Must serialize identically to the old tuple struct format
        let bare_bytes = encode_cbor(&ChatMessageContentV0("hello".into())).unwrap();
        assert_eq!(bytes, bare_bytes);

        let decoded: ChatMessageContent = decode_cbor(&bytes).unwrap();
        assert_eq!(decoded, v0);
    }

    #[test]
    fn chat_message_v1_roundtrip() {
        let v1 = ChatMessageContent::text("hello");
        let bytes = encode_cbor(&v1).unwrap();
        let decoded: ChatMessageContent = decode_cbor(&bytes).unwrap();
        assert_eq!(decoded, v1);
    }

    #[test]
    fn chat_message_getters() {
        let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
        assert_eq!(v0.message(), "hello");
        assert!(v0.media().is_none());

        let v1 = ChatMessageContent::text("world");
        assert_eq!(v1.message(), "world");
        assert!(v1.media().is_none());
    }

    #[test]
    fn version_convert_v1_to_v0() {
        use comcap::{VersionConvert, VersionConvertError};

        let v1 = ChatMessageContent::text("hello");
        let v0 = v1.to_version(0).unwrap();
        assert_eq!(v0, ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into())));
    }

    #[test]
    fn version_convert_v0_to_v1() {
        use comcap::VersionConvert;

        let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
        let v1 = v0.to_version(1).unwrap();
        assert_eq!(v1.message(), "hello");
        assert!(v1.media().is_none());
    }

    #[test]
    fn version_convert_empty_message_is_lossy() {
        use comcap::{VersionConvert, VersionConvertError};
        use crate::chat::{ChatMessageV1, ChatMessageVersions};

        let v1_media_only = ChatMessageContent::Versioned(ChatMessageVersions::V1(ChatMessageV1 {
            message: "".into(),
            media: None, // in practice would have media here
        }));
        let result = v1_media_only.to_version(0);
        assert_eq!(result, Err(VersionConvertError::Lossy));
    }

    #[test]
    fn version_convert_unknown_version() {
        use comcap::{VersionConvert, VersionConvertError};

        let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
        let result = v0.to_version(99);
        assert_eq!(result, Err(VersionConvertError::UnknownVersion));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dashchat-node chat_message_compat`

Expected: FAIL — `ChatMessageContentV0` doesn't exist yet.

- [ ] **Step 3: Refactor ChatMessageContent in message.rs**

Replace the contents of `crates/dashchat-node/src/chat/message.rs`. The key changes:

1. The old `ChatMessageContent` struct becomes `ChatMessageV1` (the V1 inner type)
2. The old `ChatMessageContent(String)` tuple struct format becomes `ChatMessageContentV0`
3. A new `ChatMessageVersions` enum wraps V1+
4. `ChatMessageContent` becomes a type alias for `Compat<ChatMessageContentV0, ChatMessageVersions>`
5. Getters are added on the Compat instantiation
6. `text()` constructor and `From<&str>` now produce V1 variants

```rust
use named_id::RenameNone;
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};

use comcap::{
    Capability, Compat, VersionConvert, VersionConvertError,
};

// --- Media types (unchanged) ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct PhotoAttachment {
    pub data: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct FileAttachment {
    pub data: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum MediaAttachment {
    #[serde(rename = "photos")]
    Photos { photos: Vec<PhotoAttachment> },
    #[serde(rename = "file")]
    File { file: FileAttachment },
}

// --- Versioned ChatMessageContent ---

/// The V0 type: original tuple struct wrapping a String.
/// Serializes as a bare CBOR string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageContentV0(pub String);

/// V1+ versions of ChatMessageContent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum ChatMessageVersions {
    #[serde(rename = "1")]
    V1(ChatMessageV1),
}

/// V1: message text with optional media.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatMessageV1 {
    pub message: String,
    pub media: Option<MediaAttachment>,
}

/// Versioned ChatMessageContent: V0 (bare string) or V1+ (tagged).
pub type ChatMessageContent = Compat<ChatMessageContentV0, ChatMessageVersions>;

impl ChatMessageContent {
    /// Create a text-only message (V1).
    pub fn text(message: impl Into<String>) -> Self {
        Compat::Versioned(ChatMessageVersions::V1(ChatMessageV1 {
            message: message.into(),
            media: None,
        }))
    }

    /// Get the message text, regardless of version.
    pub fn message(&self) -> &str {
        match self {
            Compat::Unversioned(v0) => &v0.0,
            Compat::Versioned(ChatMessageVersions::V1(v1)) => &v1.message,
        }
    }

    /// Get the media attachment, if any.
    pub fn media(&self) -> Option<&MediaAttachment> {
        match self {
            Compat::Unversioned(_) => None,
            Compat::Versioned(ChatMessageVersions::V1(v1)) => v1.media.as_ref(),
        }
    }
}

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

impl VersionConvert for ChatMessageContent {
    const CAPABILITY: Capability = Capability::Messaging;

    fn to_version(&self, target: u16) -> Result<Self, VersionConvertError> {
        match (self, target) {
            (Compat::Unversioned(_), 0) => Ok(self.clone()),
            (Compat::Versioned(ChatMessageVersions::V1(v1)), 0) => {
                if v1.message.is_empty() {
                    Err(VersionConvertError::Lossy)
                } else {
                    Ok(Compat::Unversioned(ChatMessageContentV0(
                        v1.message.clone(),
                    )))
                }
            }
            (Compat::Unversioned(v0), 1) => Ok(Compat::Versioned(ChatMessageVersions::V1(
                ChatMessageV1 {
                    message: v0.0.clone(),
                    media: None,
                },
            ))),
            (Compat::Versioned(_), 1) => Ok(self.clone()),
            _ => Err(VersionConvertError::UnknownVersion),
        }
    }
}

/// An emoji reaction to a message.
///
/// If an author creates multiple reactions to the same message, only the last one is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatReaction {
    /// The emoji to react with.
    /// Use None to "remove" the prior reaction.
    pub emoji: Option<String>,
    /// The hash of the header of the message being reacted to.
    pub target: Hash,
}

#[cfg(feature = "testing")]
pub mod testing {
    use super::*;

    use std::cmp::Ordering;

    use named_id::RenameAll;

    use crate::{Cbor, DeviceId, Header};

    /// A standalone chat message suitable for sending to the frontend.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
    pub struct ChatMessage {
        pub content: ChatMessageContent,
        pub author: DeviceId,
        pub timestamp: u64,
    }

    impl ChatMessage {
        pub fn new(content: ChatMessageContent, header: &Header) -> Self {
            Self {
                content,
                author: header.public_key.into(),
                timestamp: header.timestamp,
            }
        }
    }

    impl Cbor for ChatMessage {}

    impl PartialOrd for ChatMessage {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(
                self.timestamp
                    .cmp(&other.timestamp)
                    .then(self.content.message().cmp(other.content.message()))
                    .then(self.author.cmp(&other.author)),
            )
        }
    }

    impl Ord for ChatMessage {
        fn cmp(&self, other: &Self) -> Ordering {
            self.timestamp
                .cmp(&other.timestamp)
                .then(self.content.message().cmp(other.content.message()))
                .then(self.author.cmp(&other.author))
        }
    }
}
```

Key differences from the original:
- `ChatMessageContent` is now `Compat<ChatMessageContentV0, ChatMessageVersions>` (type alias)
- Field access `self.content.message` becomes `self.content.message()` (getter method)
- `text()` and `From<&str>` now produce V1 (the "current" version)
- The testing module's `PartialOrd`/`Ord` impls use `.message()` getter instead of `.message` field

- [ ] **Step 4: Fix compile errors in dependent code**

The main change consumers see: `content.message` (field access) → `content.message()` (method call).

**Imports:** `ChatMessageContent` is still exported from `crate::chat::*` via `crate::ChatMessageContent`, so imports in `payload.rs` and `node.rs` work unchanged. `Payload` implements `Cbor` and `AsBody` — the inner `ChatMessageContent` just needs `Serialize + Deserialize` which `Compat` provides. No changes needed to serde infrastructure.

**Frontend/Tauri commands:** In `src-tauri/src/commands/direct_chats.rs` line 14, `content: ChatMessageContent` is deserialized from JSON sent by the frontend. Since the `Compat` deserializer uses `#[serde(untagged)]` (format-agnostic), the frontend must send either:
- V0: a bare string (e.g., `"hello"`)
- V1: `{ "v": "1", "message": "hello", "media": null }`

The frontend currently sends `{ message: "...", media: ... }` which matches neither format. The frontend update is handled in Task 3.

**`AnnouncementsPayload` wildcard match:** In `crates/dashchat-node/src/node.rs` around line 462, there's a `match` on `AnnouncementsPayload` with a `_ => None` wildcard. This already covers any future variants, so no change needed when `SetCapabilities` is added later.

- [ ] **Step 5: Run all tests**

Run: `cargo test -p dashchat-node`

Expected: All tests pass, including the new compat tests and the existing `test_mailbox_late_join` test which uses `send_message` with `"Hello".into()`.

- [ ] **Step 6: Check frontend Tauri command compatibility**

The frontend currently sends messages via the `direct_chat_send_message` Tauri command. The frontend sends JSON like:
```json
{ "message": "hello", "media": null }
```

With the new type, the Tauri command's `content: ChatMessageContent` parameter will try to deserialize this JSON. The untagged deserializer will:
1. Try `ChatMessageVersions` (internally tagged, expects `"v"` key) → fails (no `"v"` field)
2. Try `ChatMessageContentV0` (expects a bare string) → fails (it's an object, not a string)

**This means the frontend must be updated to send the V1 format:**
```json
{ "v": "1", "message": "hello", "media": null }
```

Search for where the frontend constructs `ChatMessageContent` and update it.

Run: `grep -r "direct_chat_send_message\|ChatMessageContent\|send_message" packages/stores/src/ ui/src/ --include="*.ts" --include="*.svelte" -l`

Update the frontend to include `v: "1"` in the message content object.

- [ ] **Step 7: Commit**

```bash
git add crates/dashchat-node/src/chat/message.rs crates/dashchat-node/src/compat.rs crates/dashchat-node/src/compat_tests.rs crates/dashchat-node/src/lib.rs
git commit -m "feat: refactor ChatMessageContent to use Compat versioning"
```

---

### Task 3: Update frontend message sending

**Files:**
- Modify: Frontend files that construct chat message content (likely in `packages/stores/src/`)

- [ ] **Step 1: Find frontend message construction**

Run: `grep -rn "message.*media\|send_message\|ChatMessageContent" packages/stores/src/ ui/src/ --include="*.ts" --include="*.svelte" | head -30`

- [ ] **Step 2: Update message content to include version tag**

Wherever the frontend constructs a message content object like `{ message: "...", media: ... }`, add the version tag: `{ v: "1", message: "...", media: ... }`.

Update the TypeScript type definition if one exists.

- [ ] **Step 3: Verify the app works end-to-end**

Start the dev environment and send a message between two instances. Verify it arrives correctly.

- [ ] **Step 4: Commit**

```bash
git add packages/stores/ ui/src/
git commit -m "feat: update frontend to send versioned message content"
```

---

## Chunk 3: Capability Exchange

### Task 4: Add capabilities to QrCode

**Files:**
- Modify: `crates/dashchat-node/src/contact.rs:25-39` (QrCode struct)
- Modify: `crates/dashchat-node/src/contact.rs:56-81` (Display/FromStr impls)
- Modify: `crates/dashchat-node/src/contact.rs:96-121` (existing test)

- [ ] **Step 1: Write failing test — QrCode with capabilities round-trips**

Add to `crates/dashchat-node/src/contact.rs` tests module:

```rust
#[test]
fn test_contact_with_capabilities_roundtrip() {
    use comcap::{Capabilities, Capability};
    use std::collections::BTreeMap;

    let pubkey = PublicKey::from_bytes(&[11; 32]).unwrap();
    let agent_id = AgentId::from(ActorId::from_bytes(&[22; 32]).unwrap());
    let mut caps = BTreeMap::new();
    caps.insert(Capability::Messaging, 1u16);

    let contact = QrCode {
        device_pubkey: DeviceId::from(pubkey),
        inbox_topic: None,
        agent_id,
        share_intent: ShareIntent::AddContact,
        capabilities: Some(caps.clone()),
    };
    let encoded = contact.to_string();
    let decoded = QrCode::from_str(&encoded).unwrap();
    assert_eq!(contact, decoded);
}

#[test]
fn test_old_contact_without_capabilities() {
    // Simulate an old-format QR code (4-tuple without capabilities)
    let pubkey = PublicKey::from_bytes(&[11; 32]).unwrap();
    let agent_id = AgentId::from(ActorId::from_bytes(&[22; 32]).unwrap());
    let device_id = DeviceId::from(pubkey);

    // Encode as old 4-tuple format
    let bytes = encode_cbor(&(
        &device_id,
        &None::<InboxTopic>,
        &agent_id,
        &ShareIntent::AddContact,
    ))
    .unwrap();
    let hex_str = hex::encode(bytes);
    let decoded = QrCode::from_str(&hex_str).unwrap();
    assert_eq!(decoded.capabilities, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dashchat-node test_contact_with_capabilities`

Expected: FAIL — `capabilities` field doesn't exist on QrCode.

- [ ] **Step 3: Add capabilities field to QrCode**

In `crates/dashchat-node/src/contact.rs`, add to the `QrCode` struct:

```rust
#[serde(default)]
pub capabilities: Option<Capabilities>,
```

Update the `Display` impl (line 57-66) to include capabilities in the CBOR tuple:

```rust
impl std::fmt::Display for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = encode_cbor(&(
            &self.device_pubkey,
            &self.inbox_topic,
            &self.agent_id,
            &self.share_intent,
            &self.capabilities,
        ))
        .map_err(|_| std::fmt::Error)?;
        write!(f, "{}", hex::encode(bytes))
    }
}
```

Update the `FromStr` impl (line 69-81) to decode the 5-tuple, falling back to 4-tuple for old codes:

```rust
impl FromStr for QrCode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        // Try new 5-tuple format first
        if let Ok((device_pubkey, inbox_topic, agent_id, share_intent, capabilities)) =
            decode_cbor::<(DeviceId, Option<InboxTopic>, AgentId, ShareIntent, Option<Capabilities>)>(bytes.as_slice())
        {
            return Ok(QrCode {
                device_pubkey,
                inbox_topic,
                agent_id,
                share_intent,
                capabilities,
            });
        }
        // Fall back to old 4-tuple format
        let (device_pubkey, inbox_topic, agent_id, share_intent) = decode_cbor(bytes.as_slice())?;
        Ok(QrCode {
            device_pubkey,
            inbox_topic,
            agent_id,
            share_intent,
            capabilities: None,
        })
    }
}
```

Add import at top of contact.rs:
```rust
use comcap::Capabilities;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dashchat-node test_contact`

Expected: All contact tests pass (both old and new format).

- [ ] **Step 5: Commit**

```bash
git add crates/dashchat-node/src/contact.rs
git commit -m "feat: add capabilities field to QrCode for version exchange"
```

---

### Task 5: Add SetCapabilities to AnnouncementsPayload

**Files:**
- Modify: `crates/dashchat-node/src/payload.rs:41-45` (AnnouncementsPayload enum)

- [ ] **Step 1: Add the variant**

In `crates/dashchat-node/src/payload.rs`, add to `AnnouncementsPayload`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum AnnouncementsPayload {
    SetProfile(Profile),
    SetCapabilities(Capabilities),
}
```

Add `Capabilities` to the imports at the top of `payload.rs`:

```rust
use crate::{AgentId, AsBody, Cbor, ChatMessageContent, ChatReaction, Topic};
```

becomes:

```rust
use crate::{AgentId, AsBody, Capabilities, Cbor, ChatMessageContent, ChatReaction, Topic};
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p dashchat-node`

Expected: Compiles. The new variant doesn't require handling anywhere yet since `AnnouncementsPayload` is processed via serde deserialization and pattern matching — check if there are existing match arms that need a new case.

Run: `grep -rn "AnnouncementsPayload" crates/dashchat-node/src/ --include="*.rs" | grep -v "^.*:.*use\|^.*:.*enum\|^.*:.*Serialize"`

If there are match statements, add the `SetCapabilities` arm.

- [ ] **Step 3: Commit**

```bash
git add crates/dashchat-node/src/payload.rs
git commit -m "feat: add SetCapabilities variant to AnnouncementsPayload"
```

---

## Chunk 4: Integration — Version Conversion in Send Path

### Task 6: Wire up version conversion in Node::send_message

**Files:**
- Modify: `crates/dashchat-node/src/node.rs:515-532` (send_message method)

This task depends on how capabilities are stored and looked up. The full capability storage system (persisting `BTreeMap<AgentId, Capabilities>`) is a larger change that interacts with the contact store.

For the initial implementation, add a `capabilities_for` helper that returns V0 defaults (all capabilities unknown). This can be wired to real storage in a follow-up.

- [ ] **Step 1: Add a stub capabilities_for method**

In `crates/dashchat-node/src/node.rs`, add a method:

```rust
/// Look up the known capabilities for a peer.
/// Returns empty map (all V0) if unknown.
pub fn capabilities_for(&self, _peer: &AgentId) -> Capabilities {
    // TODO: look up from stored capabilities
    Capabilities::default()
}
```

- [ ] **Step 2: Update send_message to use version conversion**

Modify `send_message` in `crates/dashchat-node/src/node.rs`:

```rust
pub async fn send_message(
    &self,
    topic: impl Into<ChatId>,
    message: ChatMessageContent,
) -> anyhow::Result<Header> {
    let topic = topic.into();

    // TODO: resolve target peer from topic for capability lookup
    // For now, always send as latest version (no downgrade)
    let header = self
        .author_operation(
            topic,
            Payload::Chat(ChatPayload::Message(message.clone())),
            None,
        )
        .await?;

    Ok(header)
}
```

The actual capability-based downgrade requires knowing the target peer(s) from the topic, which involves looking up the chat's members. This is left as a TODO with the infrastructure in place — `capabilities_for` + `VersionConvert::to_version` are ready to use once the peer resolution and capability storage are implemented.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p dashchat-node`

Expected: All tests pass.

- [ ] **Step 4: Run cargo check on the full workspace**

Run: `cargo check`

This verifies `src-tauri` and other crates compile with the new types.

- [ ] **Step 5: Commit**

```bash
git add crates/dashchat-node/src/node.rs
git commit -m "feat: add capabilities_for stub and version conversion infrastructure in send path"
```

---

### Task 7: Update log redaction patterns

**Files:**
- Modify: `src-tauri/src/commands/redact_log.rs:22-23` (regex pattern)

- [ ] **Step 1: Check if the debug format changed**

The `ChatMessageContent` type changed from a struct to a `Compat` enum. Its `Debug` output will now look different. Print the new debug format:

Run: `cargo test -p dashchat-node -- --nocapture 2>&1 | head -5` or add a quick test.

The debug format of `Compat::Versioned(ChatMessageVersions::V1(ChatMessageV1 { message: "...", ... }))` is different from the old `ChatMessageContent { message: "...", ... }`.

- [ ] **Step 2: Update redaction regex**

In `src-tauri/src/commands/redact_log.rs`, update the regex pattern at line 22-23 to match both the old and new debug formats:

```rust
// Old format: ChatMessageContent { message: "...", ... }
// New format: Versioned(V1(ChatMessageV1 { message: "...", ... }))
r#"ChatMessage(?:Content|V1)\s*\{[^}]*\}"#,
```

Also update the test assertions at lines 192-198 and 261-267 to match the new debug format.

- [ ] **Step 3: Run redaction tests**

Run: `cargo test -p dashchat redact`

Expected: All redaction tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/redact_log.rs
git commit -m "fix: update log redaction patterns for versioned ChatMessageContent"
```

---

### Task 8: Final integration test

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`

Expected: All tests pass across the entire workspace.

- [ ] **Step 2: Type-check the frontend**

Run: `cd ui && pnpm check`

Expected: No type errors.

- [ ] **Step 3: Commit any remaining fixes**

If any fixes were needed, commit them.
