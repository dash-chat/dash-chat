# Mailbox iroh blob sync — design

**Date:** 2026-06-17
**Status:** Approved, ready for implementation planning

## Goal

Recent work gave nodes an iroh endpoint + iroh-blobs store + a background blob
fetch loop (`crates/dashchat-node/src/blob_sync.rs`) so they sync media blobs
peer-to-peer, outside p2panda. This design extends the same capability to
**mailboxes**: each mailbox-server gains an iroh endpoint, an iroh-blobs store,
and a fetch loop, so it can pull media blobs from clients and later serve them
to other clients.

A mailbox is **blind**: it only ever sees encrypted p2panda ops and never learns
blob hashes on its own. Therefore **clients must tell the mailbox which blob
hashes to fetch and from whom.** Clients do this by including, in their publish
request, the iroh hashes of blobs attached to the ops being published, plus the
client's own iroh `EndpointId` (a valid source for those blobs).

`MailboxId` becomes the mailbox's iroh `EndpointId`, so from the id alone a
client knows the mailbox's iroh endpoint and can dial it as a blob provider.

## Terminology rename: blob → blip

The mailbox code currently uses "blob" to mean *an encrypted p2panda log item*.
That now collides with the iroh sense of "blob" (a content-addressed binary
object). To free the word "blob" for the iroh meaning, **rename the existing
mailbox "blob" concept to "blip"** across `mailbox-server`, `mailbox-client`,
and their consumers.

This is a mechanical `s/blob/blip/` covering Rust identifiers **and** the
wire/disk surface (HTTP route paths, JSON field names, redb table name). It is a
breaking change to the mailbox protocol and storage format, which is acceptable
because the project is pre-alpha and we control both ends. It is done as an
isolated first phase with no behavior change.

Representative renames:

| Before | After |
| --- | --- |
| `Blob` | `Blip` |
| `BlobsKey` / `BlobsKeyPrefix` / `BlobsKeyError` | `BlipsKey` / `BlipsKeyPrefix` / `BlipsKeyError` |
| `StoreBlobsRequest` | `StoreBlipsRequest` |
| `GetBlobsRequest` / `GetBlobsResponse` / `GetBlobsForTopicResponse` | `GetBlipsRequest` / `GetBlipsResponse` / `GetBlipsForTopicResponse` |
| `BLOBS_TABLE` (redb string `"blobs"`) | `BLIPS_TABLE` (redb string `"blips"`) |
| files `blob.rs`, `blobs_table.rs`, `store_blobs.rs`, `get_blobs.rs` | `blip.rs`, `blips_table.rs`, `store_blips.rs`, `get_blips.rs` |
| routes `/blobs/store`, `/blobs/get` | `/blips/store`, `/blips/get` |
| fields `blobs`, `blobs_by_topic`, `blob_count`, … | `blips`, `blips_by_topic`, `blip_count`, … |

After this rename, "blob" in the mailbox crates refers unambiguously to iroh
blobs.

## Architecture

### Server: identity, endpoint, blob store

On startup, `mailbox_server::spawn_server`:

1. Loads-or-generates a 32-byte iroh secret key from a **new single-row redb
   table** `SERVER_KEY_TABLE` in the existing db (generated on first start).
2. Builds an `iroh::Endpoint` from that key, with mDNS discovery enabled.
3. Builds an `iroh_blobs::store::fs::FsStore` + `BlobsProtocol`, accepting the
   blobs ALPN via `endpoint.accept_unmixed(iroh_blobs::ALPN, blobs)` — mirroring
   `BlobSync::new`.
4. Exposes its `EndpointId` so clients can use it as the `MailboxId` (via the
   health endpoint for the cloud case, and the mDNS announcement for the local
   case).

A new module `crates/mailbox-server/src/blob_sync.rs` holds a **minimal,
payload-agnostic fetch loop** (deliberately duplicated from the node's rather
than shared, to avoid coupling the server to `dashchat-node`'s `Payload`):

- A `BlobFetchPool` keyed `Hash → set<EndpointId>` — every client that published
  an op carrying a given hash is recorded as a source for it.
- `fetch_loop` / `run_fetch_pass` modeled on the node's loop in
  `crates/dashchat-node/src/blob_sync.rs` (concurrency, attempt timeout, pass
  interval, wake-on-add).
- `try_fetch(hash)` downloads the blob once, trying all known source
  `EndpointId`s via `Shuffled`, and removes the hash from the pool on success.

`AppState` gains the blobs protocol, the fetch pool, and the endpoint handle.
`mailbox-server/Cargo.toml` gains `iroh` and `iroh-blobs` at the workspace
versions.

### Request API

`StoreBlipsRequest` (formerly `StoreBlobsRequest`) gains fields alongside the
existing per-topic/author/seq map of encrypted blips:

```rust
pub struct StoreBlipsRequest {
    pub blips: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
    /// Iroh hashes of blobs attached to the ops in this request.
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    /// The publishing client's iroh EndpointId — a source for those blobs.
    pub sender_pubkey: iroh::EndpointId,
    /// Signature over the request by the sender's key. Carried but NOT verified
    /// yet — opaque placeholder for future authentication.
    pub signature: Vec<u8>,
}
```

The store handler records each `(hash → sender_pubkey)` into the server's
`BlobFetchPool` (adding the pubkey as a source for the hash) and notifies the
fetch loop so it begins pulling. Hashes are sent flat (not per-op): the server
only needs hash+source pairs to fetch, not an op association.

iroh blobs themselves live in the iroh `FsStore`, **not** redb. redb continues
to hold only encrypted blips, watermarks, and now the server key.

### Client & MailboxId

- `ToyMailboxClient` gains the owning node's iroh `EndpointId`. When it
  `publish`es ops, it walks them for attached media hashes
  (`MediaMetaItem.hash`) and sends them in `StoreBlipsRequest.blob_hashes` with
  `sender_pubkey = own EndpointId` and an opaque `signature`.
- **`MailboxId` = the mailbox's iroh `EndpointId`.** The cloud
  `ToyMailboxClient::new` takes the mailbox's `EndpointId`; the mDNS path puts
  the mailbox's `EndpointId` in the announcement (replacing the
  device-id-derived instance name) so discovery yields it directly.
- Node side: `MixedSourceLookup::sources` un-comments
  `sources.extend(self.mailboxes.get_sources(log_id).await?)`. A new
  `Mailboxes::get_sources(log_id)` returns the `EndpointId`s of mailboxes
  currently tracking that topic, so a recipient node will try the mailbox as a
  blob provider when the original sender is unavailable.

## Data flow

1. Alice sends a media message. The op (carrying media metadata incl. iroh
   hashes) is published to the mailbox via `StoreBlipsRequest`, which now also
   carries `blob_hashes` + Alice's `EndpointId`.
2. The mailbox records `hash → Alice's EndpointId` in its fetch pool; its fetch
   loop downloads the blob from Alice (discoverable via mDNS while she is up).
3. Alice goes offline.
4. Bobbi comes online and syncs the op from the mailbox. His blob fetch loop
   resolves sources for the hash via `MixedSourceLookup`; Alice is gone, so the
   mailbox (now a recorded source via `get_sources`) is the provider. Bobbi
   downloads the blob from the mailbox.

## Error handling

- Missing/corrupt server key row → generate a fresh key (log a warning). Key
  generation is idempotent on first start.
- Fetch failures (source unreachable, timeout) are handled exactly as in the
  node loop: the hash stays in the pool and is retried on the next pass; other
  recorded sources are tried via `Shuffled`.
- A blob the mailbox never manages to fetch simply isn't served; the recipient
  retries later or, if the original sender returns, fetches from them directly.
- The opaque `signature` field is not validated; a malformed value has no
  effect beyond being ignored.

## Testing

### Server unit tests

In `crates/mailbox-server/src/blob_sync.rs`, mirror the node's fetch-loop unit
tests (`crates/dashchat-node/src/blob_sync.rs`): empty pool parks, one pass
drains succeeding items, failing item retried after ~one interval, add wakes the
loop early, concurrency limit respected. Adapt to the `Hash → set<EndpointId>`
pool shape.

### End-to-end integration test

New `tokio::test(flavor = "multi_thread")` in `crates/dashchat-node/tests/`,
using a **real** `ToyMailbox` over HTTP with a real iroh endpoint, mDNS
discovery **on**. To prove the blob travels client→mailbox→client and never
client→client directly, the two nodes are **never online simultaneously**:

1. Bring Alice online with the mailbox; send a media message; wait for the
   mailbox to fetch the blob from Alice.
2. Drop Alice's node.
3. Bring Bobbi online with the mailbox; wait for Bobbi to sync the op and then
   download the blob — the only remaining source is the mailbox.
4. Assert `load_media` on Bobbi returns the original bytes.

Because Alice and Bobbi are never up together, no direct node-to-node p2panda or
blob sync can occur; the mailbox is provably the relay.

## Implementation phases (sequenced, one plan)

1. **Rename blob → blip** across mailbox crates + consumers (wire + disk),
   no behavior change. Build + existing tests green.
2. **Server endpoint + key + blob store**: redb `SERVER_KEY_TABLE`, iroh
   endpoint from key, `FsStore` + `BlobsProtocol`, `AppState` wiring, expose
   `EndpointId`. iroh/iroh-blobs deps added.
3. **Server fetch loop**: `mailbox-server/src/blob_sync.rs` with
   `Hash → set<EndpointId>` pool + loop + `try_fetch`, plus unit tests.
4. **Request API**: extend `StoreBlipsRequest` with `blob_hashes`,
   `sender_pubkey`, opaque `signature`; store handler records sources + wakes
   the loop.
5. **Client changes**: `ToyMailboxClient` sends hashes + pubkey; `MailboxId =
   EndpointId` (cloud ctor + mDNS announce/discovery); `Mailboxes::get_sources`
   + un-comment in `MixedSourceLookup::sources`.
6. **End-to-end integration test** (staged-online model above).

## Open risk

The mDNS announcement currently derives the local mailbox's instance name from
the device id (`src-tauri/src/mailbox/server.rs::mdns_service_info`). Switching
`MailboxId` to the iroh `EndpointId` means the announce side must publish the
endpoint id and the browse side (`src-tauri/src/mailbox.rs`) must consume it as
the id. Resolve the exact carrier (instance name vs. TXT record) during phase 5;
the EndpointId is 32 bytes / 64 hex chars, which fits a single DNS label.
