# Mailbox scrubbing

Follow-up to delete-for-everyone. When a message is deleted for everyone, every
node that processes the delete drops the payload locally and never transmits it
again. The mailboxes, however, keep serving the original operation — body and
all — until the retention window expires (90 days for blips, 7 days for blobs).
A recipient that syncs from a mailbox before it has processed the delete still
receives the payload, and a late joiner receives it indefinitely.

Scrubbing closes that gap: as soon as a delete is processed, the mailbox copy of
each deleted operation is replaced with the same operation minus its payload,
and the associated media blobs are dropped from blob storage.

## Current state

- A **blip** is `cbor(MailboxOperation { topic, header, body })` — see
  `crates/dashchat-node/src/mailbox.rs`. The mailbox treats it as opaque bytes.
- Blips live in `BLIPS_TABLE`, keyed by `BlipsKey { topic_id, author,
  sequence_number, uuid }`. `author` is the p2panda device public key.
- Media blobs are stored separately, announced by hash via
  `/blobs/register-hashes` and either uploaded inline or fetched by the mailbox.
  **The mailbox has no association between a blob and the blip that references
  it** — the blip body is encrypted, so it cannot learn one.
- Node-side, `enforce_tombstone` already drops the body of a tombstoned op, and
  `OpStore`'s `MailboxStore::get_log` serves tombstoned ops body-less. So every
  node that has seen a deleted op can already produce its payload-free form
  byte-for-byte.
- The mailbox is unauthenticated: `/blips/store`, `/blips/get`, `/blobs/*` are
  callable by anyone. `StoreBlipsRequest` carries `sender_pubkey` and
  `signature` fields that are currently ignored.

`crates/dashchat-node/tests/delete_messages.rs` already contains the acceptance
assertions behind a `mailbox_scrubbing_implemented()` stub returning `false`.

## Validation model

The mailbox cannot decode a blip, so it cannot decide for itself what a
"payload-free version" of one looks like. Instead the publisher commits to it up
front:

> When storing a blip, the client also sends `scrub_hash` — the blake3 hash of
> the *same operation with its body removed*. Later, anyone may present those
> payload-free bytes to a scrub endpoint; the mailbox accepts the replacement
> only if it hashes to the committed value.

This gives one invariant, enforced without the mailbox understanding anything
about the payload:

**The only mutation a stored blip can undergo is the one its own publisher
pre-authorized — replacement by its payload-free form.**

An attacker cannot forge, alter, or substitute content. Scrubbing is idempotent
(the scrubbed bytes still hash to the commitment) and order-independent, so any
number of nodes can scrub the same blip concurrently.

### What this does not defend against

Constructing the payload-free bytes requires the operation's header, which
anyone who can fetch the topic's blips already has. So **scrub authority is
equivalent to read authority**: anyone who can read a topic from the mailbox can
also censor it there. Likewise, `/blobs/scrub` can carry no validation at all —
the mailbox has no idea which blip a blob belongs to — so **anyone who knows a
blob hash can delete it from the mailbox**, which again is the same capability
required to read it.

The damage is bounded: payloads are E2E encrypted, recipients who already synced
keep their copies, and the loss is availability for not-yet-synced recipients
only. For an unauthenticated toy mailbox this is an acceptable trade; it should
be revisited when the mailbox gains real authentication.

**Optional strengthening** (see open questions): `BlipsKey.author` *is* the
authoring device's public key, so the mailbox could additionally require an
ed25519 signature over the scrubbed bytes verifiable under that key. That
restricts scrubbing to the operation's author, at the cost of preventing
recipients — and the author's other devices — from scrubbing on their behalf.

## Design

### 1. Commitment at store time

`StoreBlipsRequest` gains a second map, mirroring the shape of `blips`:

```rust
pub struct StoreBlipsRequest {
    pub blips: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
    #[serde(default)]
    pub scrub_hashes: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, ScrubHash>>>,
    // ... existing sender_pubkey, signature
}
```

A parallel optional map rather than changing the `Blip` value type, so the wire
format stays compatible in *both* directions: an old client's request still
deserializes on a new server, and a new client's request still deserializes on
an old server (which ignores the commitment, leaving the blip unscrubbable
rather than failing the store outright). Given that mailbox servers and clients
update independently, a store request that hard-fails against a lagging server
would stop message delivery entirely — not worth the tidier shape.

`ScrubHash` is blake3 (`iroh_blobs::Hash`, already in both crates' dependency
graphs).

Commitments are stored in a new table, keyed by the same `BlipsKey` as the blip
itself:

```rust
pub const SCRUB_TABLE: TableDefinition<BlipsKey, &[u8]> =
    TableDefinition::new("scrub_commitments");
```

A separate table rather than widening `BLIPS_TABLE`'s value type, which would be
a redb schema change requiring migration of existing databases. `cleanup.rs`
deletes from both tables in the same pass.

Blips stored without a commitment (old clients, or rows predating this change)
are simply unscrubbable. Bounded by the retention window.

### 2. `POST /blips/scrub`

```jsonc
// request
{ "blips": { "<topic>": { "<author>": { "<seq>": "<base64 payload-free blip>" } } } }

// response 200
{ "scrubbed": [["<topic>", "<author>", 7]], "rejected": [["<topic>", "<author>", 9]] }
```

Per `(topic, author, seq)`:

1. Range `BLIPS_TABLE` over the `TopicAuthorSeq` prefix (a seq can hold more
   than one row — `store_blips` inserts a fresh UUID each time).
2. For each row, read its `SCRUB_TABLE` commitment. No commitment → skip.
3. `blake3(submitted_bytes) == commitment` → overwrite the row's value with the
   submitted bytes. Otherwise → reject that entry.

The key is left in place, so watermarks and the `missing` computation in
`/blips/get` are untouched — this is why scrubbing replaces rather than deletes.
No push notifications are emitted.

### 3. Once scrubbed, always scrubbed

`store_blips` must not let a node that has not yet processed the delete
resurrect a payload. Before inserting at `(topic, author, seq)`, it checks
whether an existing row there is already scrubbed and skips the insert if so.

"Already scrubbed" is *derived*, not flagged: a row is scrubbed exactly when its
stored bytes hash to its own commitment. No extra state to keep in sync, and an
op that never had a body is trivially and harmlessly "already scrubbed".

In practice this path is rarely hit — the mailbox reports the seq as present, so
clients do not republish it — but it closes the race.

### 4. `POST /blobs/scrub`

```jsonc
{ "blob_hashes": ["<hash>", "..."] }
```

Two endpoints rather than one: blips and blobs live in different storage
subsystems (redb vs. the iroh blob store), have different request shapes, and
have different validation stories. The node-side call site issues both together.

Per hash: remove it from the `BlobFetchPool` (otherwise the mailbox re-fetches
it from a peer that still holds it), and delete its `mailbox/<secs>/<hash>`
retention tags so iroh's GC reclaims the bytes on its next sweep.

### 5. Node-side triggering

- `MailboxItem` gains `fn scrubbed(&self) -> Option<Self> { None }`;
  `MailboxOperation` overrides it with `Self { topic, header, body: None }`. The
  default keeps in-memory and test item types untouched.
- `MailboxClient` gains `scrub_blips` / `scrub_blobs`, defaulting to `Ok(())` so
  the in-memory client is unaffected. `ToyMailboxClient` implements both.
- `Mailboxes` gains a `scrub` method fanning out to every tracked mailbox.
- The trigger is `enforce_tombstone`: whenever it returns `true`, the node has
  just dropped a body, so it holds exactly the op needed to scrub and (via
  `unprocess_app`, which already extracts them) the media hashes to release.
  Every node that processes the delete scrubs every mailbox it knows —
  consistent with the existing "every node keeps every mailbox in sync"
  philosophy, and idempotent, so redundancy is free.
- Because the author's own delete flows through the same `enforce_tombstone`
  path, author-side scrubbing needs no separate call site.

**Retry story:** fire-and-forget, self-healing. If a scrub does not land, the
mailbox keeps serving the body; the next node to sync it re-ingests the op,
`enforce_tombstone` fires again, and the scrub is re-issued. No new persistence.
Retention is the ultimate backstop.

A node that only ever saw the delete, never the deleted op, cannot scrub — it
has no header to submit. That is fine; nodes that did see it will.

## Testing

- **mailbox-server** (`tests/integration.rs`): store-with-commitment then scrub;
  scrub with wrong bytes rejected; scrub of an uncommitted blip rejected; scrub
  twice is idempotent; re-store after scrub does not resurrect the payload;
  `/blips/get` returns the blip body-less afterwards; watermarks unchanged.
- **blob**: after `/blobs/scrub` the mailbox no longer serves the blob and does
  not re-fetch it.
- **dashchat-node**: delete `mailbox_scrubbing_implemented()` and its
  `#[deprecated]` marker in `tests/delete_messages.rs`, enabling the three
  already-written blocks — including the late-joiner (carol) assertion that a
  node syncing purely from the mailbox never receives a deleted payload.

## Open questions

1. **Author signature?** Require an ed25519 signature over the scrubbed bytes
   under `BlipsKey.author`, or accept that read authority implies scrub
   authority? Signing blocks third-party censorship but also blocks recipients
   and the author's other devices from scrubbing.
2. **Re-fetch suppression for blobs.** A node racing the delete can re-announce
   a scrubbed blob's hash via `/blobs/register-hashes` and the mailbox will
   fetch it again. Add a short-lived "recently scrubbed" set the announce path
   consults, or rely on the next scrub to clean it up?
3. **Wire compatibility.** Is the parallel `scrub_hashes` map the right call, or
   is a cleaner value type worth a coordinated client/server rollout?
4. **Determinism.** Publisher and scrubber must produce byte-identical CBOR for
   the payload-free operation. Any change to `MailboxOperation`'s serde
   representation silently makes in-flight blips unscrubbable (bounded by
   retention). Worth a round-trip test pinning the encoding?
