# Encrypting media blobs (design considerations)

Status: design notes — nothing implemented yet.

Goal: whenever an operation refers to a media blob, the blob is encrypted with
the same keys that the operation *will* (later) be encrypted with, i.e. the
p2panda space's group secret. The blob's content address becomes the hash of
the **ciphertext**, so identical media sent in different contexts (different
spaces, or after a key rotation) has different hashes. Operations themselves
stay plaintext for now.

## Where things stand today

- Blobs are plaintext everywhere: in the local iroh-blobs `FsStore`
  (`crates/dashchat-node/src/blob_sync.rs`), over the iroh-blobs protocol to
  peers, over plain HTTP `POST /blobs/upload` to mailboxes, and at rest on
  mailbox servers for up to 7 days.
- The blob identity is `iroh_blobs::Hash` = BLAKE3 over the stored bytes
  (the BAO root). The iroh downloader and the mailbox server both verify
  against whatever bytes are stored, so "hash of ciphertext" falls out
  naturally once we store ciphertext: **no mailbox-server changes are needed**.
- Operations reference blobs only through `MediaMetadata`
  (`crates/dashchat-node/src/chat/message.rs`): `{name, mime_type, size,
  width/height / duration/waveform, hash}`. No key material, no nonce.
- Media only ever appears in `ChatPayload::Message` bodies, and both direct
  chats and group chats are spaces, so every blob-referencing operation *does*
  have a space (and therefore a group secret) behind it. `Profile.avatar` and
  `GroupInfo.image` are inline `Option<String>`s today, not blob refs — but
  profiles travel over inbox and device-group topics, which are **not**
  spaces; if avatars ever become blobs this design needs a key source for
  them (see "Non-space topics" below).
- Spaces run the p2panda-encryption **data scheme**: XChaCha20-Poly1305 with a
  symmetric `GroupSecret`, kept in a `SecretBundle` that retains every past
  secret. A new secret is created on space creation and on member **removal**
  (add does not rotate; invitees are welcomed with the whole bundle, so late
  joiners can read history). `SpacesArgs::SpaceUpdate` (manual rotation) is
  `unimplemented!()` in the fork.
- Although spaces are hooked up, dash-chat still publishes through the raw
  `StreamPublisher` (`TopicStreams::publish`), not `Space::publish`, so the
  group secrets exist but are unused for app payloads. That's exactly the key
  material blobs should use.

## The encryption scheme

### Key source

Use the space's current `GroupSecret` — the same secret `Space::publish` /
`EncryptionGroup::send` would use for the operation. The committed state is
reachable via `SpacesManager::get_space_state(&space_id)` →
`SpaceState.encryption_y.secrets.latest()`.

Practical obstacles, all in the fork (which we control):

- `get_space_state` is public, but dash-chat reaches the `SpacesManager` only
  through test-gated, deprecated paths (`p2panda::Node::spaces_manager()` is
  `#[cfg(any(test, feature = "test_utils"))]`; `TopicStreams::with_node` is
  `#[deprecated]`). We already lean on this for `has_space`/`space_members`,
  but blob encryption makes it a production dependency — the fork should grow
  a real API.
- `GroupSecret::as_bytes()` is `pub(crate)`. The only exposed primitive is
  `encrypt_data(plaintext, &GroupSecret, nonce) -> Vec<u8>` /
  `decrypt_data(...)` (XChaCha20-Poly1305, 24-byte nonce). That's actually a
  good shape: we never need raw key bytes outside the crate.

Proposed fork addition: something like
`Space::encrypt_blob(&[u8]) -> (GroupSecretId, XAeadNonce, Vec<u8>)` and
`Space::decrypt_blob(GroupSecretId, XAeadNonce, &[u8]) -> Vec<u8>`, or an
equivalent on the manager, so dash-chat never touches secrets directly and the
test_utils/deprecated paths can stay test-only.

**Domain separation (decide):** using the `GroupSecret` directly for blobs
means blob ciphertexts and (future) operation ciphertexts share a key. AEAD
with random nonces makes that safe, but deriving a blob subkey
(`HKDF(group_secret, "dashchat-blob/v1")`) inside the fork API is cheap
insurance and would later allow handing out a blob key without exposing the
operation key (e.g. forwarding, out-of-band attachment serving).

### Pin the secret, don't assume "latest"

"The key the operation *will* be encrypted with" is racy: a member removal
between encrypting the blob and publishing the op rotates the secret, and on
the receive side an op can arrive before (or long after) the rotation it
interleaves with. So the blob must carry **which** secret was used:

- Record the `GroupSecretId` (= SHA-256 of the secret, `[u8;32]`) in
  `MediaMetadata`, exactly as `SpacesArgs::Application { group_secret_id,
  nonce, ciphertext }` does for ops.
- Decrypt with `SecretBundle::get(&id)`, never `latest()`. Since the bundle
  retains all past secrets and welcomes ship the whole bundle, old blobs stay
  decryptable after rotations and for late joiners — no re-encryption on
  membership change, matching message-history semantics.
- Consequence (same as for messages): a removed member who saved old secrets
  can still decrypt blobs they can obtain. Blob GETs are unauthenticated (any
  peer who knows a hash can fetch), so removal protects *future* media only.

### Nonce (decide)

Two options:

1. **Random 24-byte nonce per blob**, stored in `MediaMetadata`. Simple,
   matches what the data scheme does for ops. Cost: sending the same file
   twice in the same chat produces two ciphertexts → two blobs stored,
   uploaded, fetched.
2. **Derived nonce** (e.g. `HKDF(group_secret, blake3(plaintext))[..24]`):
   convergent *within a key context*. Same file + same secret → same
   ciphertext → same hash, restoring dedup inside a chat (and the nonce field
   could even be omitted if derivation inputs are all recoverable — they
   aren't on the receive side, so it still has to be stored or re-derived
   after decrypt; simpler to store it either way). Leaks equality of blobs to
   chat members, who can decrypt anyway, and to observers *within one context*
   (an observer can tell "this chat sent the same attachment twice"). Probably
   acceptable; cross-context unlinkability is preserved because the secret
   differs.

Either way nonce handling lives next to the fork API so misuse isn't possible
from dash-chat.

## Wire-format changes (`MediaMetadata`)

Each variant (`Photo`, `File`, `VoiceNote`) needs, alongside the existing
ciphertext `hash`:

- `group_secret_id: [u8; 32]`
- `nonce: [u8; 24]` (unless derived — see above; recommend storing it
  regardless)
- Decide what `size` means. Ciphertext = plaintext + 16-byte Poly1305 tag.
  The UI wants plaintext size; transfer caps and progress want ciphertext
  size. Cleanest: keep `size` = plaintext size (UI compatibility) since
  ciphertext size is always derivable (+16).

**Do not add a plaintext hash.** It would be tempting (end-to-end integrity of
the decrypted bytes, dedup detection), but operations are still plaintext, so
a plaintext hash in the metadata is a content fingerprint visible to mailboxes
and relays — precisely the leak this step closes (known-file matching). The
AEAD tag already authenticates the plaintext under the group key. Revisit once
ops are encrypted.

Compatibility:

- `MediaMetadata` is CBOR inside the op body (with the `Compat` buffering and
  the `hash_bytes` str/bytes/seq dance). New required fields are a hard format
  break for old clients; `#[serde(default)]` fields would make old blobs
  "metadata without keys" and new blobs unreadable by old clients anyway.
- Given pre-alpha status, a hard cutover (`ChatMessageContentV2` or new
  required fields, old clients show nothing/error) is probably fine, but it's
  a decision. If negotiation is wanted, `dashchat_compat::capabilities!` is
  the existing mechanism; note the e2e compat suite (`e2e-tests/compat/`)
  will pin whatever we choose.
- Frontend mirror types in `packages/stores/src/types.ts`
  (`MediaMetadata`) must gain the fields only if the webview ever needs them —
  see "Decrypt point" below for why it ideally doesn't.

## Write path (send)

Order matters, because the op embeds the ciphertext hash:

1. Resolve the space + current `GroupSecret` for the chat topic. (Node-side,
   in `store_media` / `send_message`, `crates/dashchat-node/src/node.rs`.)
2. Encrypt each media item; compute nothing ourselves — `add_bytes` on the
   iroh store returns the ciphertext hash.
3. Build `MediaMetadata` with ciphertext hash + secret id + nonce; publish the
   op as today.
4. Tagging (`topic‖author‖op_hash‖blob_hash`), retagging, the unfetched-blobs
   table, mailbox announce/upload all operate on the ciphertext hash with no
   structural change.

Failure mode to define: **pending spaces**. A direct chat right after
`accept_contact`, or a group right after join, may not have a welcomed
encryption state yet (`GroupNotYetEstablished` / `NoGroupSecretAvailable`).
Today `send_message` succeeds immediately. Options: block the send with a
user-visible error, or queue media sends until the space is established. This
same problem hits op encryption later, so whatever we build (probably "wait
for established space with timeout") should be shared.

Size caps: `MAX_BLOB_BYTES` (16 MiB, `crates/dashchat-utils/src/blob_sync.rs`)
is enforced on what's stored/downloaded — now ciphertext. The UI's
`MAX_MESSAGE_BYTES` (16 MiB) checks plaintext. Add headroom for the +16 bytes
per item (e.g. cap plaintext at `MAX_BLOB_BYTES - 16`) or bump the transfer
cap; otherwise a maximal attachment fails only after encryption.

## Read path (receive/display)

Fetching is untouched: `process_app` queues ciphertext hashes, the fetch pool
downloads with BAO verification against the ciphertext hash, mailbox fallback
works as today. The changes are at the consumption points, which all funnel
through `Node::load_blob(hash)`:

- `irohblob://<hash>` protocol handler (`src-tauri/src/blob_protocol.rs`)
- `save_blob_to_cache` (`src-tauri/src/commands/media.rs`)
- `load_media` (node.rs)

**Decrypt point: keep it in the node.** The webview should keep addressing
blobs by (ciphertext) hash alone — `irohblob://hash` URIs, `convertFileSrc`,
the gallery, caching semantics all stay untouched, and key material never
crosses the IPC boundary. That requires the node to resolve
`hash → (group_secret_id, nonce, space)` on its own:

- Maintain a local index populated when ops are processed (`process_app`)
  and when we send (`store_media`) — either a sqlite table in `localdata.db`
  next to `unfetched_blob_hashes`, or rebuilt from ops at startup the way
  `BlobFetchPool::from_ops` already does. The op-derived rebuild is
  attractive: no new persistent state, and deletes come for free with
  tombstones.
- `load_blob` becomes `fetch ciphertext → look up (secret_id, nonce) → get
  space secrets → decrypt_data → plaintext`, with a distinct error for "blob
  present but secret not yet available" (op/blob can arrive before the space
  welcome). The UI's existing retry/cache-buster machinery
  (`blob-load-store.svelte.ts`) handles "not yet" states already; decrypt
  failures must not count toward the fetch pool's `MAX_FETCH_FAILURES`
  eviction (the fetch succeeded; it's a key problem).
- Everything downstream of decrypt is unchanged: content-type sniffing,
  `?decode=wav` Opus decode, image rendering — they just see plaintext bytes
  as today. The `Cache-Control: immutable` header stays valid: ciphertext
  hash → plaintext mapping is deterministic.
- `save_blob_to_cache` still writes **plaintext** to the OS cache dir so
  external viewers can open it. That's a deliberate plaintext-at-rest hole
  (as is the entire local iroh store today for our own media — local at-rest
  encryption is out of scope for this step, but worth a line in the threat
  model).

## Dedup, storage, deletion consequences

- **Cross-context dedup is gone by design**: same photo in two chats = two
  local blobs, two uploads, two mailbox copies. Forwarding media between
  chats becomes decrypt + re-encrypt + re-upload. Local storage grows
  accordingly; acceptable, but the "forward to chat" feature (when it comes)
  must go through the node, not just copy a hash.
- **Mailbox global dedup disappears — and that's a feature.** Today
  `/blobs/register-hashes` answers `already_stored`, which is a
  confirmation-of-file oracle across all users, and the server can read every
  attachment. After this change the server stores opaque ciphertext and
  hashes are unlinkable across contexts.
- Deletion gets *more* correct: today the same plaintext hash in two chats
  shares one blob + per-op tags; per-context hashes mean a tombstone in one
  chat can never race another chat's copy.
- Mailbox 7-day blob retention semantics are unchanged.

## What this step does and does not protect (threat model)

Protected: blob **content** from mailbox servers, relaying peers, and anyone
who learns a blob hash (blob GETs are unauthenticated).

Still exposed until ops are encrypted (worth stating explicitly since it
bounds how much this step buys):

- The whole `MediaMetadata` sits in a plaintext op body: file **name**,
  mime type, plaintext size, image dimensions, voice-note duration and
  waveform, plus the message text, sender, topic. The waveform in particular
  is a real content leak for voice notes.
- `group_secret_id` and nonce are visible (harmless cryptographically, but
  they fingerprint rotation events).
- Ciphertext length ≈ plaintext length (no padding). Padding schemes are a
  possible later refinement; Signal pads attachments.

## Interaction with the next step (encrypting operations)

- `MailboxOperation::blob_hashes()` (`crates/dashchat-node/src/mailbox.rs`)
  decodes the op **body** to find blob hashes, and it runs client-side on both
  publish and ingest — the mailbox server never decodes bodies. So op
  encryption doesn't break blob announcement structurally, but the ingesting
  client must decrypt before it can learn which blobs to fetch; the
  hash-resolution index above should be fed from decrypted payloads.
- When ops move to `Space::publish`, p2panda routes them to
  `application_log_id(space_id)` (digest of `space_id ++
  "space_application/v1"`) instead of `LogId::from_topic(topic)`, which
  dash-chat assumes in several places (op projections, blob source lookup,
  mailbox tests). Blob encryption doesn't force that migration, but don't
  build anything new that hard-codes the current log-id assumption.
- Choosing the same `(group_secret_id, nonce)` encoding as
  `SpacesArgs::Application` keeps the two steps symmetrical.

## Non-space topics

Inbox topics and the device group are plain streams with no encryption state.
Nothing references blobs from them today, but:

- Contact requests/accepts carry `Profile` (with inline avatar string) and key
  bundles in plaintext over inboxes — unaffected by this step.
- If avatars or group images become blobs, the group-image case is fine (group
  chats are spaces) but profile avatars published to the device group or
  inboxes would have **no key**. Either keep avatars inline, or scope avatar
  blobs per-space (re-encrypted into each chat), or give the device group a
  space (there's already a TODO about the device-group topic needing rework).

## Redaction / reporting

`REDACTION_REGEXES` (`src-tauri/src/commands/redact_log.rs`) covers hashes and
byte arrays, which should already catch ciphertext hashes, secret ids and
nonces in logs — verify, and make sure the new fork API never logs key
material. Never log plaintext blob bytes (already the rule).

## Testing

- Rust: `crates/dashchat-node/tests/{blob_sync, mailbox_blob_sync,
  photo_delivery, delete_messages}.rs` and
  `crates/mailbox-server/tests/integration.rs` all exercise blob flow and will
  need the metadata changes threaded through. New coverage to add:
  - mailbox server stores bytes that are not the plaintext (e.g. don't sniff
    as JPEG, differ from input);
  - same plaintext in two chats yields two different hashes, both decrypt;
  - late joiner decrypts pre-join media; member removed → rotation → old media
    still decrypts for remaining members, new media uses new secret id;
  - blob arrives before space welcome → decrypt retries and succeeds;
  - decrypt failure doesn't evict the blob from the fetch pool.
- E2E: `media-attachments.spec.ts` / `media-stress.spec.ts` should pass
  unchanged (the UI contract is stable) — which is itself the regression test
  that decryption is transparent.

## Decisions to make before implementing

1. Random stored nonce vs derived (convergent-per-context) nonce.
2. `GroupSecret` used directly vs HKDF blob subkey (recommend subkey).
3. Hard wire-format break vs `Compat`/capability negotiation (pre-alpha
   suggests hard break).
4. Pending-space sends: fail, or queue until established.
5. Shape of the fork API (`Space::encrypt_blob`-style vs exposing secret
   bundle reads) and un-deprecating a production path to the spaces manager.
