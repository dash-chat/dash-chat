# Versioned Payload Compatibility

**Date:** 2026-03-11
**Status:** Draft

## Problem

Dash Chat serializes payloads as CBOR and sends them over p2panda's append-only operation logs. As the app evolves, payload types change shape — e.g., `ChatMessageContent` was originally a tuple struct wrapping a `String`, and now includes optional media attachments. Older clients can't deserialize the new format, and newer clients need to understand the old format.

We need a system that:
- Lets any data type opt in to versioning without forcing complexity on types that don't need it
- Ensures all versions are handled at compile time (exhaustive match)
- Preserves exact wire compatibility with pre-versioning clients for V0
- Supports downgrade so newer clients can talk to older clients
- Works through mailboxes and relays (no direct handshake required for correctness)

## Design

### Core Concepts

- **V0**: The original, pre-versioning format. Serializes bare (no wrapper), identical to what old clients produce and expect.
- **V1+**: Explicitly versioned. Serializes as `{ "v": "<version>", "d": <data> }`.
- **Capability**: A named feature domain (e.g., `Messaging`) that groups related types. Each capability has a version number representing the highest version a client supports.
- **Downgrade**: Converting a payload to an older version for compatibility with a peer.

### `Compat<Bare, Tagged>` — The Generic Wrapper

A single generic enum handles the bare-vs-tagged wire format for all versioned types. Its `Serialize`/`Deserialize` implementation is written once.

```rust
/// Transparent versioning wrapper.
/// - `Bare`: the V0 type (serialized without any wrapper)
/// - `Tagged`: an enum of V1+ versions (serialized as `{ "v": "N", "d": ... }`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compat<Bare, Tagged> {
    Unversioned(Bare),
    Versioned(Tagged),
}
```

**Serialization:**
- `Unversioned(bare)` → serializes `bare` directly (no wrapper)
- `Versioned(tagged)` → delegates to `tagged`'s own `Serialize`, which uses `#[serde(tag = "v", content = "d")]`

**Deserialization:**
1. Attempt to deserialize as `Tagged` (expects a CBOR map with both `"v"` AND `"d"` keys — both must be present)
2. If that fails, deserialize as `Bare` (V0 fallback)

**Constraint:** The `Bare` type must not serialize as a CBOR map containing both `"v"` and `"d"` keys at the top level, as this would collide with the versioned envelope format. In practice this is unlikely — V0 types predate versioning and were not designed with these field names — but implementers should verify when adding versioning to a type.

**Forward compatibility:** If a client receives a `Tagged` payload with an unknown version (e.g., a V1 client receives `{ "v": "2", "d": ... }`), the `Tagged` enum's serde deserialization will fail (no matching variant). The `Compat` deserializer then falls back to trying `Bare`, which will also fail if the wire format is not the V0 shape. The operation is skipped. The application layer should handle per-operation deserialization failures gracefully (log a warning, skip the operation) rather than failing the entire topic sync. The UI should indicate that unsupported message types were received, prompting the user to upgrade.

Old clients that predate versioning:
- Successfully read V0 messages (identical wire format)
- Fail to deserialize V1+ messages (unrecognized structure) — correct behavior since they don't support those features

### Per-Type Versioning (Example: `ChatMessageContent`)

The original `ChatMessageContent` was a tuple struct wrapping a `String` (serialized as a bare CBOR string). The current version adds media attachments.

```rust
/// The versioned type alias — drop-in replacement for the original type.
type ChatMessageContent = Compat<ChatMessageContentV0, ChatMessageVersions>;

/// The original V0 type — a newtype around String.
/// Serializes as a bare CBOR string, identical to the old tuple struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageContentV0(pub String);

/// V1+ versions, each as an enum variant.
/// Serde tags handle wire discrimination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "v", content = "d")]
enum ChatMessageVersions {
    #[serde(rename = "1")]
    V1(ChatMessageV1),
    // #[serde(rename = "2")]
    // V2(ChatMessageV2),  // added in the future
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ChatMessageV1 {
    message: String,
    media: Option<MediaAttachment>,
}
```

**Wire format examples:**
- V0: `"hello"` (bare CBOR string — identical to original `ChatMessageContent(String)`)
- V1: `{ "v": "1", "d": { "message": "hello", "media": null } }`

**Getter methods** materialize a consistent view across versions:

```rust
impl Compat<ChatMessageContentV0, ChatMessageVersions> {
    fn message(&self) -> &str {
        match self {
            Self::Unversioned(v0) => &v0.0,
            Self::Versioned(ChatMessageVersions::V1(v1)) => &v1.message,
        }
    }

    fn media(&self) -> Option<&MediaAttachment> {
        match self {
            Self::Unversioned(_) => None,
            Self::Versioned(ChatMessageVersions::V1(v1)) => v1.media.as_ref(),
        }
    }
}
```

Adding getters for each version variant ensures the compiler enforces exhaustive handling whenever a new version is added.

### Opting In

Versioning is opt-in per type:

1. **Unversioned types** (default): Use plain structs with `Serialize`/`Deserialize` as today. No changes needed.
2. **Versioned types**: Replace the type with `Compat<OriginalType, VersionsEnum>`. The original type becomes the V0 bare format.

The change is local to the type and its slot in the parent enum (e.g., `ChatPayload::Message`). No changes are needed to the `Payload` enum, `Cbor` trait, `AsBody`, operation authoring, gossip encoding, or any other infrastructure.

### Capability System

#### `Capability` Enum

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    Messaging,
    // Future: VoiceCalls, Groups, etc.
}

/// Maps each capability to the highest version the client supports.
/// Absence of a capability implies V0.
pub type Capabilities = BTreeMap<Capability, u16>;
```

Using an enum instead of strings ensures type safety — the compiler catches typos and stale references.

**Capability version semantics:** A capability version bump represents a coordinated change across all types in that capability group. For example, `Messaging` version 1 means "supports `ChatMessageContent` V1 (with media)". If `ChatReaction` later needs a V1, that would bump `Messaging` to version 2, meaning "supports `ChatMessageContent` V1 + `ChatReaction` V1". All types within a capability advance in lockstep — a client that claims `Messaging: 2` must support all type versions up to that level.

#### When Capabilities Are Learned

1. **Contact exchange (initial):** The `QrCode` struct gains an optional `capabilities: Option<Capabilities>` field. Old clients produce QR codes without it → parsed as empty map → all V0. New clients include their full capability set.

2. **Announcements topic (updates):** A new variant in `AnnouncementsPayload`:
   ```rust
   pub enum AnnouncementsPayload {
       SetProfile(Profile),
       SetCapabilities(Capabilities),
   }
   ```
   Published when a client upgrades. All peers subscribed to that agent's announcements topic receive the update.

3. **Groups (future):** The effective capability set for a group is the component-wise minimum version across all members. When any member publishes `SetCapabilities`, the group's effective set is recomputed.

#### Storage

Each node maintains a `BTreeMap<AgentId, Capabilities>` populated from contact exchange and updated via announcements. Persisted alongside contact data.

If a peer's capabilities are unknown (e.g., a contact added before versioning was implemented), all capabilities are assumed to be V0.

### Version Conversion

#### `VersionConvert` Trait

```rust
pub trait VersionConvert: Sized {
    /// Which capability governs this type's versioning.
    const CAPABILITY: Capability;

    /// Convert self to a representation at the target version.
    /// Returns Err if the content can't be meaningfully represented
    /// at that version (e.g., a media-only message to a V0 peer).
    fn to_version(&self, target_version: u16) -> Result<Self, VersionConvertError>;
}

pub enum VersionConvertError {
    /// The content can't be represented at the target version without
    /// unacceptable data loss (e.g., media-only message → V0).
    Lossy,
    /// The target version is not recognized.
    UnknownVersion,
}
```

Version conversion is hand-written per type because lossy conversion logic is inherently type-specific.

#### Example: `ChatMessageContent`

```rust
impl VersionConvert for ChatMessageContent {
    const CAPABILITY: Capability = Capability::Messaging;

    fn to_version(&self, target: u16) -> Result<Self, VersionConvertError> {
        match (self, target) {
            (Self::Unversioned(_), 0) => Ok(self.clone()),
            (Self::Versioned(ChatMessageVersions::V1(v1)), 0) => {
                if v1.message.is_empty() {
                    Err(VersionConvertError::Lossy)
                } else {
                    Ok(Self::Unversioned(ChatMessageContentV0(v1.message.clone())))
                }
            }
            (Self::Unversioned(v0), 1) => {
                Ok(Self::Versioned(ChatMessageVersions::V1(
                    ChatMessageV1 { message: v0.0.clone(), media: None }
                )))
            }
            (Self::Versioned(_), 1) => Ok(self.clone()),
            _ => Err(VersionConvertError::UnknownVersion),
        }
    }
}
```

#### Where Version Conversion Happens

In the node layer, before `author_operation` is called:

```rust
impl Node {
    fn send_message(&self, peer: AgentId, content: ChatMessageContent) -> Result<()> {
        let peer_caps = self.capabilities_for(peer);
        let messaging_v = peer_caps.get(&Capability::Messaging).copied().unwrap_or(0);
        let sendable = content.to_version(messaging_v)?;
        self.author_operation(topic, Payload::Chat(ChatPayload::Message(sendable)), None)
    }
}
```

For groups, `messaging_v` is the minimum across all group members.

When `to_version` returns `VersionConvertError::Lossy`, the UI can inform the user that the message can't be sent to that peer/group in its current form.

#### Group Downgrade Trade-offs

Because p2panda operations are append-only and immutable, each operation is authored once to a group topic and seen by all members. This means a group must use a single version per message — you can't send V1 to some members and V0 to others.

The design uses **lowest-common-denominator**: the group's effective version for a capability is the minimum across all members. This means one lagging member degrades the experience for everyone (e.g., no media messages if one member is V0).

This is a conscious trade-off:
- **Alternative: send highest version, let old clients degrade gracefully.** This would give newer members the full experience, but old clients would see "unsupported message" placeholders instead of readable content. For a messaging app, unreadable messages are worse than reduced features.
- **Alternative: dual-publish at multiple versions.** This doubles storage and bandwidth, adds complexity, and still doesn't help old clients read V1+ content.

The LCD approach is the right default. The UI should surface when a group is constrained (e.g., "Media messages are unavailable — a group member hasn't upgraded yet") so users can nudge lagging members. As the app matures and old versions age out, this constraint naturally relaxes.

### Graceful Handling of Unknown Payloads

Adding new enum variants (e.g., `SetCapabilities` to `AnnouncementsPayload`) means old clients will encounter unknown `"type"` values during deserialization. The operation processing pipeline must handle per-operation deserialization failures gracefully — skip the unrecognized operation and continue syncing rather than failing the entire topic.

This is a prerequisite for this spec and should be verified or implemented before rolling out any new payload variants.

### QrCode Backward Compatibility

The `capabilities` field added to `QrCode` requires `#[serde(default)]` to ensure old QR codes (without the field) deserialize correctly:

```rust
pub struct QrCode {
    pub device_pubkey: DeviceId,
    pub agent_id: AgentId,
    pub inbox_topic: Option<InboxTopic>,
    pub share_intent: ShareIntent,
    #[serde(default)]
    pub capabilities: Option<Capabilities>,
}
```

Old QR codes without the field will deserialize with `capabilities: None`, which is treated as "all V0" — the correct assumption for pre-versioning clients.

### Integration Points

The versioning system is encapsulated and does not affect the following layers:
- `Payload` enum structure
- `Cbor` / `AsBody` traits
- Operation authoring (`author_operation`)
- Gossip wire encoding (`encode_gossip_message`)
- p2panda operation storage and sync
- Frontend store/event system

Changes are limited to:
- The versioned types themselves (local refactor)
- `QrCode` struct (add optional `capabilities` field)
- `AnnouncementsPayload` (add `SetCapabilities` variant)
- Node's send paths (capability lookup + downgrade call)
- A new `Capabilities` store on each node

### Adding a New Version (Checklist)

When a type needs a new version:

1. Add a new struct for the version (e.g., `ChatMessageV2`)
2. Add a variant to the versions enum (e.g., `ChatMessageVersions::V2`)
3. Update getter methods — compiler will enforce this via exhaustive match
4. Update `VersionConvert` impl with conversion logic for the new version
5. Bump the version number for the relevant `Capability`
6. The node will publish `SetCapabilities` on next startup with the new version

### Adding Versioning to a New Type (Checklist)

When a previously unversioned type needs versioning:

1. Create a versions enum with the V1 variant containing the new shape
2. Replace the type with `Compat<OriginalType, VersionsEnum>`
3. Add getter methods on the `Compat` instantiation
4. Implement `VersionConvert`
5. Either add a new `Capability` variant or increment an existing one if the type belongs to an existing capability group
