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

The fork API proposal is in "Fork API" below; it is designed around the
requirement that, once operations are encrypted too, blob and operation must
use the same secret atomically.

**Domain separation (decided):** blobs are encrypted with an HKDF subkey of
the group secret (fixed info label, e.g. `"blob/v1"`), not with the group
secret directly. AEAD with random nonces would make direct use safe, but the
subkey separates blob ciphertexts from (future) operation ciphertexts and
would later allow handing out a blob key without exposing the operation key
(e.g. forwarding, out-of-band attachment serving). The derivation lives in
p2panda-encryption so `GroupSecret::as_bytes()` stays crate-private.

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

### Nonce (decided)

**Random 24-byte `XAeadNonce` per blob**, stored in `MediaMetadata`, exactly
as the data scheme does for operations (`SpacesArgs::Application { ...,
group_secret_id, nonce, ciphertext }`). Blob encryption is fully symmetric
with operation encryption: same `GroupSecretId` and `XAeadNonce` types, same
serde encoding; the only differences are the HKDF blob subkey and that the
ciphertext lives in the blob store instead of the message. Each blob in a
multi-attachment message gets its own nonce (same secret). Cost accepted:
sending the same file twice in the same chat produces two ciphertexts (no
convergent dedup within a context). Nonce generation lives inside the fork
API so misuse isn't possible from dash-chat.

## Fork API

The constraint that shapes the API: once operations are encrypted too, the
blob has to be encrypted *first* (its ciphertext hash goes into the
operation's payload), yet blob and operation must use the **same** group
secret, atomically with respect to rotations. The API we add now for
blob-only encryption should be the restriction of that future API, not a
throwaway.

### Where atomicity already lives in p2panda

- `p2panda_spaces::Space::publish` runs its whole read → modify → write cycle
  inside `manager.begin_write()`, a `StateTransaction` that holds a store
  permit from `store.begin()` (`p2panda-spaces/src/store.rs`). The fork
  already switched the sqlite store to `BEGIN IMMEDIATE`, so an open write
  transaction excludes all concurrent writers.
- Secret rotations are state writes through that same `begin_write`: remote
  removals arrive via `Manager::process`, local ones via `Space::remove`.
- Inside `apply_publish` (`p2panda-spaces/src/space.rs`) the state is a plain
  value (`space_y`) read once from the transaction, and
  `EncryptionGroup::send` takes `secrets.latest()` from it.

So anything encrypted from the same `space_y` snapshot, within the same write
transaction as the forged operation, is structurally guaranteed to use the
operation's secret. No new locking is needed — the API just has to put blob
encryption inside that existing boundary.

### Layered proposal

**p2panda-encryption** (`data_scheme`), alongside `encrypt_data` /
`decrypt_data`:

```rust
pub fn encrypt_blob(plaintext: &[u8], group_secret: &GroupSecret, nonce: XAeadNonce) -> Result<Vec<u8>, _>;
pub fn decrypt_blob(ciphertext: &[u8], group_secret: &GroupSecret, nonce: XAeadNonce) -> Result<Vec<u8>, _>;
```

Identical to `encrypt_data` except the AEAD key is the HKDF blob subkey of
the group secret. `GroupSecret::as_bytes()` stays `pub(crate)`.

**p2panda-spaces** — the end state, with operations encrypted:

```rust
pub struct EncryptedBlob {
    pub group_secret_id: GroupSecretId,
    pub nonce: XAeadNonce,
    pub ciphertext: Vec<u8>,
}

impl Space {
    pub async fn publish_with_blobs(
        &self,
        blobs: Vec<Vec<u8>>,
        payload: impl FnOnce(&[EncryptedBlob]) -> Result<Vec<u8>, _>,
    ) -> Result<(F::Message, Event<C>, Vec<EncryptedBlob>), SpaceError<F, C>>;
}
```

Flow, inside one `begin_write` (an extended `apply_publish`):

1. Read `space_y`; take `secrets.latest()`.
2. Encrypt each blob with the subkey and a fresh random nonce.
3. Hand the `EncryptedBlob`s to the `payload` closure. The caller computes
   each iroh hash right there — `iroh_blobs::Hash::new(&ciphertext)` is pure
   BLAKE3, no store round-trip — builds `MediaMetadata`, and serializes the
   op payload referencing them.
4. `EncryptionGroup::send` on those payload bytes. Same `space_y` value, so
   the same secret as step 2 by construction.
5. Forge `SpacesArgs::Application`, commit, return the message plus the
   `EncryptedBlob`s for the caller to write into the blob store and announce.

The closure shape (rather than a two-phase `begin_publish()` guard object) is
deliberate: a guard would hold the store's write permit across arbitrary
caller code and awaits, which invites deadlocks against the node's own
processing loop; the closure keeps the transaction scope inside the library
and makes the "same secret" property impossible to break from outside. The
closure is synchronous and CPU-only, and XChaCha20 at 16 MiB is tens of
milliseconds, so holding the write transaction across it is acceptable
(membership changes are rare).

**Interim version (this step, operations still plaintext):**

```rust
impl Space {
    pub async fn encrypt_blobs(&self, blobs: Vec<Vec<u8>>) -> Result<Vec<EncryptedBlob>, _>;
    pub async fn decrypt_blob(&self, group_secret_id: GroupSecretId, nonce: XAeadNonce, ciphertext: &[u8]) -> Result<Vec<u8>, _>;
}
```

- `encrypt_blobs` takes the latest secret and encrypts all of a message's
  attachments under it (one secret, per-blob nonces). Dash-chat then builds
  the plaintext op and publishes via the stream publisher as today. Because
  the op is plaintext, there is no op key to be atomic *with* yet; the
  metadata pins `group_secret_id`, so decryption is correct across any
  rotation race. The residual (temporary) gap: a removal processed between
  `encrypt_blobs` and the publish means a blob a removed member could
  decrypt — a milliseconds-wide window that `publish_with_blobs` closes for
  good. For that reason `encrypt_blobs` can read committed state
  (`manager.read()`) instead of holding a write transaction; the stronger
  guarantee only exists in the combined call anyway.
- `decrypt_blob` resolves the secret **by id** via `SecretBundle::get` (never
  `latest()`), derives the subkey, decrypts. Its error type must distinguish
  "unknown secret id" (welcome/rotation not synced yet → retriable, feeds the
  wait-for-secrets behaviour) from AEAD failure (corrupt or hostile blob →
  permanent).
- Both are exposed on the high-level `p2panda::spaces::Space<M>` handle (and
  usable on dash-chat's joined `Space` wrapper via `Space::run`), which also
  retires dash-chat's production dependency on the test-gated
  `Node::spaces_manager()` / deprecated `TopicStreams::with_node` path.

Note that even `publish_with_blobs` cannot defend against a removal that
other members process concurrently (our publish and their removal race in the
distributed sense) — the guarantee is the local one: once *this* node has
processed a removal, nothing new is encrypted to the old secret. That is the
same semantic Signal/MLS-style systems provide.

One ordering wrinkle: `p2panda::spaces::Space::publish` currently dispatches
the forged message to the egress pipeline before returning, so with
`publish_with_blobs` the op can hit the network milliseconds before the
ciphertexts are in the local iroh store. Remote fetch retries make this
harmless, but if we want strict "blobs stored before op visible", the
high-level wrapper needs a hook between forge/commit and dispatch (or it
returns the message for the caller to dispatch, like the lower layer does).

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
- **Decided: hard format break** (pre-alpha). New required fields; old
  clients fail to render new media and that's acceptable. No
  `dashchat_compat::capabilities!` negotiation for this. The e2e compat suite
  (`e2e-tests/compat/`) will pin the new format going forward.
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
Today `send_message` succeeds immediately. **Decided: wait for the space's
encryption state to become established, failing the send with a user-visible
error on timeout.** This same problem hits op encryption later, so the wait
mechanism should be shared (the spaces manager's `EventWaiters` already
notifies on state changes; a bounded wait on "welcomed + has secret" can hang
off that rather than polling).

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

## Decisions

1. **Nonce:** random, stored in `MediaMetadata`, encoding fully symmetric
   with `SpacesArgs::Application`'s `(group_secret_id, nonce)`.
2. **Key:** HKDF blob subkey of the `GroupSecret`, derived inside
   p2panda-encryption.
3. **Wire format:** hard break, no capability negotiation.
4. **Pending spaces:** wait for established encryption state, fail the send
   on timeout.
5. **Fork API:** layered as in "Fork API" above — `encrypt_blob`/
   `decrypt_blob` primitives in p2panda-encryption; `EncryptedBlob`,
   `Space::encrypt_blobs`/`decrypt_blob` now and `Space::publish_with_blobs`
   when ops get encrypted, with the payload-builder closure running inside
   the publish write transaction so blob and op share one secret atomically.

Still open:

- Whether the high-level `Space::publish` wrapper should store blobs before
  dispatching the op (see the ordering wrinkle in "Fork API").
- Exact HKDF info label and where `EncryptedBlob` lives (p2panda-spaces vs
  p2panda-encryption).
- Whether the interim `encrypt_blobs` should bother taking a write
  transaction (stronger interim guarantee) or read committed state.
