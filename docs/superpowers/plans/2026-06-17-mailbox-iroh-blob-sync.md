# Mailbox iroh blob sync — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each mailbox-server an iroh endpoint, iroh-blobs store, and a background blob fetch loop so it can relay media blobs between clients that are never online at the same time.

**Architecture:** Clients tell the (blind) mailbox which iroh blob hashes to fetch and from whom by extending the publish request with `blob_hashes` + `sender_pubkey`. The mailbox records `hash → set<EndpointId>` and its fetch loop downloads each blob, trying all known sources. Recipients later download those blobs from the mailbox itself (the mailbox is registered as a blob source via `Mailboxes::get_sources`). First, the pre-existing mailbox "blob" concept (encrypted log item) is renamed to "blip" to free the word "blob" for the iroh meaning.

**Tech Stack:** Rust, axum (mailbox HTTP server), redb (mailbox storage), iroh + iroh-blobs (p2p blob transfer), p2panda (node networking), tokio, reqwest.

## Global Constraints

- iroh version: `1.0.0-rc.1` (workspace), `default-features = false`, features `["tls-ring"]`. iroh imports must match the p2panda iroh dep version. Copy verbatim from root `Cargo.toml`.
- iroh-blobs version: `0.102.0` (workspace).
- A node's `DeviceId` bytes ARE its iroh `EndpointId`: `iroh::EndpointId::from_bytes(device_id.as_bytes())`. This is already relied on in `crates/dashchat-node/src/blob_sync.rs::MixedSourceLookup::sources`.
- The mailbox accepts the iroh-blobs ALPN via `endpoint.accept_unmixed(iroh_blobs::ALPN, blobs.clone())` (mirror `BlobSync::new`).
- Run Rust tests with `cargo nextest run`. CI commands run inside `nix develop`.
- The `signature` field on the request is an opaque `Vec<u8>` placeholder — carried, never validated.
- Rename is a full `s/blob/blip/` across mailbox crates INCLUDING wire (HTTP routes, JSON field names) and disk (redb table name). Acceptable break: pre-alpha.
- Write very few comments (see CLAUDE.md). No `left`/`right` CSS (not relevant here — backend only).
- The fetch-loop control logic is shared: it lives once in `dashchat-utils` behind the `FetchStack` trait (Task 4) and is consumed by both `dashchat-node` and `mailbox-server`. Do NOT reintroduce a second copy of `fetch_loop`/`run_fetch_pass` in either consumer.
- Commit after each task.

---

## Phase 1 — Rename blob → blip (mailbox encrypted log items)

This phase is a pure rename with **no behavior change**. The existing test suite must stay green. It is mechanical but touches wire + disk names.

### Task 1: Rename `blob` → `blip` in the mailbox crates only

**CRITICAL SCOPE WARNING — two senses of "blob".** This rename applies ONLY to the *mailbox encrypted-log-item* concept, which lives entirely in `crates/mailbox-server` and (via consumption of its types) in `crates/mailbox-client/src/toy.rs`. The words `blob`/`Blob` ALSO appear in `crates/dashchat-node` and `src-tauri` referring to a completely different, unrelated concept: **iroh blobs** (the media-sync feature this whole project builds on — `BlobSync`, `blob_sync.rs`, `blobs_store_path`, `MediaMeta` hashes, `src-tauri/src/blob_protocol.rs`). Those MUST stay named "blob". **Do NOT run the rename over `dashchat-node` or `src-tauri`.** Verified: `dashchat-node` does not reference `mailbox_server` at all, and the only out-of-`mailbox-server` consumer of the mailbox `Blob`/`StoreBlobsRequest`/`GetBlobsRequest`/`GetBlobsResponse` types is `crates/mailbox-client/src/toy.rs`. Neither `mailbox-server` nor `mailbox-client` depends on `iroh`/`iroh_blobs` yet at this point, so a blanket `s/blob/blip/` confined to those two crates is safe and complete.

**Files (all under repo root):**
- Rename: `crates/mailbox-server/src/blob.rs` → `crates/mailbox-server/src/blip.rs`
- Rename: `crates/mailbox-server/src/blobs_table.rs` → `crates/mailbox-server/src/blips_table.rs`
- Rename: `crates/mailbox-server/src/store_blobs.rs` → `crates/mailbox-server/src/store_blips.rs`
- Rename: `crates/mailbox-server/src/get_blobs.rs` → `crates/mailbox-server/src/get_blips.rs`
- Modify: every remaining `.rs` under `crates/mailbox-server/` (src + tests) and `crates/mailbox-client/` that contains a `blob`/`Blob`/`BLOB` token.

**Interfaces:**
- Produces: `Blip`, `BlipsKey`, `BlipsKeyPrefix`, `BlipsKeyError`, `BLIPS_TABLE`, `StoreBlipsRequest`, `GetBlipsRequest`, `GetBlipsResponse`, `GetBlipsForTopicResponse`, `store_blips`, `get_blips_for_topics`, routes `/blips/store` and `/blips/get`, redb table name string `"blips"`.

- [ ] **Step 1: Confirm the pre-rename build and tests are green**

Run: `cargo nextest run -p mailbox-server -p mailbox-client`
Expected: PASS (records the green baseline before the rename).

- [ ] **Step 2: Rename the four files with `git mv`**

```bash
cd crates/mailbox-server/src
git mv blob.rs blip.rs
git mv blobs_table.rs blips_table.rs
git mv store_blobs.rs store_blips.rs
git mv get_blobs.rs get_blips.rs
cd -
```

- [ ] **Step 3: Apply the mechanical identifier rename — confined to the two mailbox crates**

Run from the repo root. The scope is `crates/mailbox-server` and `crates/mailbox-client` ONLY (NOT `dashchat-node`, NOT `src-tauri`). Case-sensitive substitutions cover the compound identifiers (`Blob`→`Blip` handles `BlobsKey`→`BlipsKey`, etc.).

```bash
FILES=$(grep -rIl --include='*.rs' -e 'blob' -e 'Blob' -e 'BLOB' \
  crates/mailbox-server crates/mailbox-client)

for f in $FILES; do
  sed -i \
    -e 's/Blob/Blip/g' \
    -e 's/blob/blip/g' \
    -e 's/BLOB/BLIP/g' \
    "$f"
done
```

This also renames the route strings (`"/blobs/store"`→`"/blips/store"`), the redb table name (`TableDefinition::new("blobs")`→`"blips"`), module declarations (`mod blob;`→`mod blip;`), and base64 helper module names. That is intended.

- [ ] **Step 4: Verify no `blob` tokens remain in the two mailbox crates, and that NOTHING outside them changed**

```bash
# (a) the mailbox crates must be fully renamed:
grep -rIn --include='*.rs' -e 'blob' -e 'Blob' -e 'BLOB' \
  crates/mailbox-server crates/mailbox-client
# Expected: NO output.

# (b) nothing outside the two mailbox crates may have changed:
git status --porcelain | grep -vE '^.. crates/mailbox-(server|client)/' || true
# Expected: NO output. If dashchat-node or src-tauri appear here, the sed
# escaped its scope — revert those files (git checkout -- <path>) before
# continuing; their 'blob' is the iroh sense and must not be renamed.
```

- [ ] **Step 5: Build**

Run: `cargo build -p mailbox-server -p mailbox-client -p dashchat-node`
Expected: PASS. `dashchat-node` and `src-tauri` are unchanged but still depend on `mailbox-client`; building them confirms the rename didn't alter `mailbox-client`'s public surface that they use (`ToyMailboxClient::new`, `MailboxItem`, etc. — none of which change in this task).

- [ ] **Step 6: Run the full affected test suite**

Run: `cargo nextest run -p mailbox-server -p mailbox-client -p dashchat-node`
Expected: PASS, identical to the Step 1 baseline (same number of tests, just renamed).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(mailbox): rename blob -> blip to free 'blob' for iroh

Renames the mailbox encrypted-log-item concept from 'blob' to 'blip'
across mailbox-server and mailbox-client (including HTTP route paths,
JSON field names, and the redb table name). No behavior change. Frees
'blob' for the upcoming iroh-blobs sense. The iroh-blob code in
dashchat-node/src-tauri is intentionally left untouched.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 2 — Server iroh key, endpoint, and blob store

### Task 2: Add iroh deps and a persistent server key

**Files:**
- Modify: `crates/mailbox-server/Cargo.toml`
- Create: `crates/mailbox-server/src/server_key.rs`
- Modify: `crates/mailbox-server/src/lib.rs` (declare `mod server_key;`, create the key table in `init_db`)

**Interfaces:**
- Produces:
  - `pub const SERVER_KEY_TABLE: redb::TableDefinition<'static, (), &'static [u8]>` (single-row table keyed by unit).
  - `pub fn load_or_create_secret_key(db: &redb::Database) -> Result<iroh::SecretKey, String>` — reads the 32-byte key, generating + persisting one if absent.

- [ ] **Step 1: Add iroh + iroh-blobs to mailbox-server deps**

Edit `crates/mailbox-server/Cargo.toml`, in `[dependencies]`, add (copy versions verbatim from root `Cargo.toml`):

```toml
iroh = { workspace = true }
iroh-blobs = { workspace = true }
```

If `mailbox-server` is not yet a workspace member that inherits these, instead add the explicit versions matching root:

```toml
iroh = { version = "1.0.0-rc.1", default-features = false, features = ["tls-ring"] }
iroh-blobs = "0.102.0"
```

Run: `cargo build -p mailbox-server`
Expected: PASS (deps resolve).

- [ ] **Step 2: Write the failing test for key persistence**

Create `crates/mailbox-server/src/server_key.rs`:

```rust
use redb::{Database, TableDefinition};

pub const SERVER_KEY_TABLE: TableDefinition<'static, (), &'static [u8]> =
    TableDefinition::new("server_key");

/// Load the persisted iroh secret key, generating and storing a new one on first use.
pub fn load_or_create_secret_key(db: &Database) -> Result<iroh::SecretKey, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(dir.path().join("test.redb")).unwrap();
        let txn = db.begin_write().unwrap();
        {
            let _ = txn.open_table(SERVER_KEY_TABLE).unwrap();
        }
        txn.commit().unwrap();
        std::mem::forget(dir);
        db
    }

    #[test]
    fn key_is_stable_across_calls() {
        let db = temp_db();
        let k1 = load_or_create_secret_key(&db).unwrap();
        let k2 = load_or_create_secret_key(&db).unwrap();
        assert_eq!(k1.to_bytes(), k2.to_bytes());
    }
}
```

Add `mod server_key;` and `pub use server_key::{load_or_create_secret_key, SERVER_KEY_TABLE};` to `crates/mailbox-server/src/lib.rs`. Ensure `tempfile` is available as a dev/test dependency (it is already used behind `test_utils`; if the unit test cannot see it, add `tempfile` under `[dev-dependencies]`).

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo nextest run -p mailbox-server server_key`
Expected: FAIL (panics at `todo!()`).

- [ ] **Step 4: Implement `load_or_create_secret_key`**

Replace the `todo!()` body:

```rust
pub fn load_or_create_secret_key(db: &Database) -> Result<iroh::SecretKey, String> {
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn
        .open_table(SERVER_KEY_TABLE)
        .map_err(|e| e.to_string())?;
    if let Some(bytes) = table.get(()).map_err(|e| e.to_string())? {
        let arr: [u8; 32] = bytes
            .value()
            .try_into()
            .map_err(|_| "stored server key is not 32 bytes".to_string())?;
        return Ok(iroh::SecretKey::from_bytes(&arr));
    }
    drop(table);
    drop(read_txn);

    let key = iroh::SecretKey::generate(&mut rand::rngs::OsRng);
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn
            .open_table(SERVER_KEY_TABLE)
            .map_err(|e| e.to_string())?;
        table
            .insert((), key.to_bytes().as_slice())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(key)
}
```

If `iroh::SecretKey::generate` requires a different RNG signature for this iroh version, use `iroh::SecretKey::generate(rand::rngs::OsRng)` or the version's documented constructor; add `rand` to `[dependencies]` if not present.

- [ ] **Step 5: Create the key table in `init_db`**

In `crates/mailbox-server/src/lib.rs`, inside `init_db`'s write transaction block (next to `open_table(BLIPS_TABLE)` and `open_table(WATERMARKS_TABLE)`), add:

```rust
        let _server_key_table = write_txn.open_table(SERVER_KEY_TABLE)?;
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p mailbox-server server_key`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(mailbox): persist a stable iroh secret key in redb

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task 3: Build the iroh endpoint + blobs store and expose the EndpointId

**Files:**
- Create: `crates/mailbox-server/src/blob_sync.rs` (endpoint + store construction lives here; the fetch loop is added in Task 4)
- Modify: `crates/mailbox-server/src/lib.rs` (declare `mod blob_sync;`, extend `AppState`, build the endpoint in `spawn_server`, add EndpointId to `HealthResponse`)

**Interfaces:**
- Consumes: `load_or_create_secret_key` (Task 2).
- Produces:
  - `pub struct BlobSync { pub blobs: iroh_blobs::BlobsProtocol, pub endpoint: iroh::Endpoint, downloader: iroh_blobs::api::downloader::Downloader, /* fetch_pool added in Task 4 */ }`
  - `pub async fn BlobSync::new(secret_key: iroh::SecretKey, root: std::path::PathBuf) -> anyhow::Result<BlobSync>`
  - `pub fn BlobSync::endpoint_id(&self) -> iroh::EndpointId`
  - `AppState.blob_sync: BlobSync`
  - `HealthResponse.endpoint_id: String` (hex of the mailbox's EndpointId).

- [ ] **Step 1: Write the failing test for endpoint construction**

Create `crates/mailbox-server/src/blob_sync.rs`:

```rust
use std::path::PathBuf;

use iroh_blobs::api::downloader::Downloader;

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub endpoint: iroh::Endpoint,
    downloader: Downloader,
}

impl BlobSync {
    pub async fn new(secret_key: iroh::SecretKey, root: PathBuf) -> anyhow::Result<Self> {
        todo!()
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_id_matches_secret_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate(&mut rand::rngs::OsRng);
        let expected = key.public();
        let bs = BlobSync::new(key, dir.path().to_path_buf()).await.unwrap();
        assert_eq!(bs.endpoint_id(), expected);
    }
}
```

Add to `lib.rs`: `mod blob_sync;` and `pub use blob_sync::BlobSync;`.

NOTE: confirm the exact methods for this iroh version: the endpoint's id accessor may be `endpoint.id()` or `endpoint.node_id()`, and the public key from a secret key may be `key.public()`. Adjust `endpoint_id()` and the test's `expected` to match. The node side uses `iroh::EndpointId`; keep that type.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: FAIL (panics at `todo!()`).

- [ ] **Step 3: Implement `BlobSync::new`**

Replace the `todo!()` body, mirroring `crates/dashchat-node/src/blob_sync.rs::BlobSync::new` but building the endpoint directly (the node received a `p2panda::Endpoint`; here we build a raw iroh one):

```rust
    pub async fn new(secret_key: iroh::SecretKey, root: PathBuf) -> anyhow::Result<Self> {
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .bind()
            .await?;

        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        endpoint
            .accept_unmixed(iroh_blobs::ALPN, blobs.clone())
            .await?;
        let downloader = Downloader::new(&store, &endpoint);

        Ok(Self {
            blobs,
            endpoint,
            downloader,
        })
    }
```

NOTE: the discovery builder name (`discovery_n0`, `discovery_local_network`, or an mDNS-specific one) and whether `accept_unmixed` exists on a raw `iroh::Endpoint` (vs. the p2panda wrapper) must be confirmed against this iroh version. If `accept_unmixed` is p2panda-only, use iroh's router: build an `iroh::protocol::Router` that registers `iroh_blobs::ALPN → blobs.clone()` and keep the router handle in `BlobSync`. The goal is identical: the mailbox both serves blobs (provider) and can download them (Downloader). If the test only needs the endpoint id, this detail is exercised more fully in Task 4 / the integration test.

- [ ] **Step 4: Run the test**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: PASS.

- [ ] **Step 5: Wire `BlobSync` into `AppState` and `spawn_server`, expose EndpointId on health**

In `crates/mailbox-server/src/lib.rs`:

Add to `AppState`:
```rust
    pub blob_sync: BlobSync,
```

Change `HealthResponse`:
```rust
#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    endpoint_id: String,
}
```

Change `health_check` to read the id from state, encoded as a `MailboxId` (base64url, no pad — see the encoding helper below):
```rust
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        endpoint_id: crate::encode_mailbox_id(state.blob_sync.endpoint_id()),
    })
}
```
(Recall `health_check` currently takes no args; add the `State` extractor and keep the `get(health_check)` route.)

**Canonical `MailboxId` encoding (base64url, no pad).** The `MailboxId` is the mailbox's iroh `EndpointId` (32 bytes) encoded as URL-safe base64 without padding — 43 chars, which fits a single mDNS DNS label (63-byte limit) and is identical in the cloud `/health` and local mDNS paths. Add to `crates/mailbox-server/src/lib.rs` (and re-export):

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// Encode an iroh EndpointId as the canonical MailboxId string (base64url, no pad).
pub fn encode_mailbox_id(id: iroh::EndpointId) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

/// Parse a MailboxId string back into an iroh EndpointId.
pub fn decode_mailbox_id(s: &str) -> anyhow::Result<iroh::EndpointId> {
    let bytes = URL_SAFE_NO_PAD.decode(s)?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("MailboxId is not 32 bytes"))?;
    Ok(iroh::EndpointId::from_bytes(&arr)?)
}
```
`base64` is already a dependency of `mailbox-server` (used by `Blip`'s base64 serde). Confirm `id.as_bytes()` is the 32-byte form for this iroh version (adjust to `id.to_bytes()` if needed). The node-side parse helper in Task 6 (`parse_endpoint_id`) must use this same `decode_mailbox_id` logic — prefer importing `mailbox_server::decode_mailbox_id` rather than duplicating it.

In `spawn_server`, after `let db_arc = Arc::new(db);`, build the blob sync:
```rust
    let secret_key = load_or_create_secret_key(&db_arc).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let blobs_root = db_path_blobs_dir(&db_path);
    let blob_sync = BlobSync::new(secret_key, blobs_root).await?;
    tracing::info!("Mailbox iroh endpoint id: {}", blob_sync.endpoint_id());
```
where `db_path_blobs_dir` is a small helper placing the iroh store next to the db file:
```rust
fn db_path_blobs_dir(db_path: &std::path::Path) -> std::path::PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("mailbox_blobs")
}
```
Note `spawn_server`'s signature uses `db_path: PathBuf`; capture `let db_path = db_path;` is already in scope — pass `&db_path` before it is moved into `init_db`. Reorder so `init_db(db_path.clone())` is used (clone the PathBuf) so `db_path` remains available for `db_path_blobs_dir`.

Pass `blob_sync` into `create_app`:
```rust
    let app = create_app(db_arc, push_client, Arc::clone(&push_tasks), blob_sync);
```
and update `create_app`'s signature + `AppState` construction:
```rust
pub fn create_app(
    db: Arc<Database>,
    push_client: Option<Arc<PushNotificationsClient>>,
    push_tasks: Arc<tokio::sync::Mutex<JoinSet<()>>>,
    blob_sync: BlobSync,
) -> Router {
    let state = AppState { db, push_client, push_tasks, blob_sync };
    ...
}
```

- [ ] **Step 6: Update every `create_app` caller**

Find callers:
```bash
grep -rn "create_app(" crates/mailbox-server
```
For each test/util caller (e.g. `test_utils.rs`, `tests/*.rs`), build a `BlobSync` with a fresh key + tempdir and pass it. Add a test helper in `test_utils.rs`:
```rust
pub async fn test_blob_sync() -> crate::BlobSync {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = iroh::SecretKey::generate(&mut rand::rngs::OsRng);
    let bs = crate::BlobSync::new(key, dir.path().to_path_buf())
        .await
        .expect("blob sync");
    std::mem::forget(dir);
    bs
}
```
and use `test_blob_sync().await` at each `create_app` call site in tests.

- [ ] **Step 7: Build and run the full server test suite**

Run: `cargo nextest run -p mailbox-server`
Expected: PASS (existing tests now construct a `BlobSync`).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(mailbox): build iroh endpoint + blobs store, expose EndpointId

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 3 — Shared fetch loop + server fetch pool

The fetch-loop control logic (bounded concurrency, pass interval, wake-on-add, per-pass tried-set) is identical for nodes and mailboxes; only the pool storage and source resolution differ. Extract the loop into the existing `dashchat-utils` grab-bag crate behind a `FetchStack` trait — it needs no iroh/p2panda types, only tokio + the trait — then migrate the node to it (Task 4) and implement the trait for the mailbox's `hash → set<EndpointId>` pool (Task 5).

### Task 4: Extract the generic fetch loop into `dashchat-utils` and migrate the node

**Files:**
- Create: `crates/dashchat-utils/src/fetch_loop.rs`
- Modify: `crates/dashchat-utils/src/lib.rs` (declare + re-export), `crates/dashchat-utils/Cargo.toml` (add `async-trait`)
- Modify: `crates/dashchat-node/src/blob_sync.rs` (impl `FetchStack` for the existing `BlobFetchPool`; delete the local `fetch_loop`/`run_fetch_pass`/`BlobFetchConfig`; move the loop unit tests out; call the shared loop)
- Modify: `crates/dashchat-node/src/node.rs` only if it names `BlobFetchConfig` directly (kept working via a re-export alias — see Interfaces)

**Interfaces:**
- Consumes: nothing new.
- Produces (in `dashchat_utils`):
  - `pub struct FetchConfig { pub concurrency: usize, pub attempt_timeout: Duration, pub pass_interval: Duration }` with `Default` (4 / 30s / 60s).
  - `#[async_trait] pub trait FetchStack: Clone + Send + Sync + 'static { type Item: Clone + Send + 'static; type Key: Copy + Eq + std::hash::Hash + Send; fn key(item: &Self::Item) -> Self::Key; async fn is_empty(&self) -> bool; async fn next_untried(&self, tried: &HashSet<Self::Key>) -> Option<Self::Item>; async fn remove(&self, item: &Self::Item); async fn wait_for_add(&self); }`
  - `pub async fn fetch_loop<P: FetchStack, F, Fut>(pool: P, config: FetchConfig, fetch: F) where F: Fn(P::Item, Duration) -> Fut + Clone + Send + 'static, Fut: Future<Output = bool> + Send + 'static`
- The node adds `pub use dashchat_utils::FetchConfig as BlobFetchConfig;` in `crates/dashchat-node/src/blob_sync.rs` so `NodeConfig.blob_fetch: BlobFetchConfig` and every `config.blob_fetch` keep compiling unchanged.

- [ ] **Step 1: Add `async-trait` to dashchat-utils and write `fetch_loop.rs` with failing tests**

In `crates/dashchat-utils/Cargo.toml` add to `[dependencies]`: `async-trait = "0.1"` (match the version `mailbox-client` already uses — check `grep async-trait crates/mailbox-client/Cargo.toml`). The `tokio` features already include `time`, `sync`, `rt`; add `macros` and `rt-multi-thread` to `[dev-dependencies].tokio` if not present (needed for the `#[tokio::test]` macros below).

Create `crates/dashchat-utils/src/fetch_loop.rs`:

```rust
use std::collections::HashSet;
use std::hash::Hash;
use std::time::{Duration, Instant};

use tokio::task::JoinSet;

#[derive(Clone, Debug)]
pub struct FetchConfig {
    pub concurrency: usize,
    pub attempt_timeout: Duration,
    pub pass_interval: Duration,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(30),
            pass_interval: Duration::from_secs(60),
        }
    }
}

/// A pool of work items the fetch loop drains. Implementors own the storage and
/// the wake signal; the loop is otherwise generic.
#[async_trait::async_trait]
pub trait FetchStack: Clone + Send + Sync + 'static {
    type Item: Clone + Send + 'static;
    type Key: Copy + Eq + Hash + Send;

    /// The per-pass dedup key for an item (the loop never re-attempts an item
    /// whose key it already tried this pass).
    fn key(item: &Self::Item) -> Self::Key;
    async fn is_empty(&self) -> bool;
    /// The next item whose key is not in `tried`, or `None` if all are tried.
    async fn next_untried(&self, tried: &HashSet<Self::Key>) -> Option<Self::Item>;
    async fn remove(&self, item: &Self::Item);
    /// Resolves when an item is added, so the loop can wake early.
    async fn wait_for_add(&self);
}

/// Drain `pool` until cancelled. Each pass walks the stack with up to
/// `config.concurrency` fetches in flight, giving each item `attempt_timeout`.
/// An item for which `fetch` returns `true` is removed. With items still
/// outstanding the loop waits up to `pass_interval` since the pass began before
/// retrying, but a newly added item wakes it early; with an empty pool it parks.
pub async fn fetch_loop<P, F, Fut>(pool: P, config: FetchConfig, fetch: F)
where
    P: FetchStack,
    F: Fn(P::Item, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let concurrency = config.concurrency.max(1);
    loop {
        let pass_start = Instant::now();
        run_fetch_pass(&pool, concurrency, config.attempt_timeout, &fetch).await;

        if pool.is_empty().await {
            pool.wait_for_add().await;
            continue;
        }
        let elapsed = pass_start.elapsed();
        if elapsed < config.pass_interval {
            tokio::select! {
                _ = tokio::time::sleep(config.pass_interval - elapsed) => {}
                _ = pool.wait_for_add() => {}
            }
        }
    }
}

async fn run_fetch_pass<P, F, Fut>(
    pool: &P,
    concurrency: usize,
    attempt_timeout: Duration,
    fetch: &F,
) where
    P: FetchStack,
    F: Fn(P::Item, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let mut tried: HashSet<P::Key> = HashSet::new();
    let mut in_flight: JoinSet<Option<P::Item>> = JoinSet::new();
    loop {
        while in_flight.len() < concurrency {
            let Some(item) = pool.next_untried(&tried).await else {
                break;
            };
            tried.insert(P::key(&item));
            let fetch = fetch.clone();
            in_flight.spawn(async move {
                fetch(item.clone(), attempt_timeout).await.then_some(item)
            });
        }
        let Some(joined) = in_flight.join_next().await else {
            break;
        };
        if let Ok(Some(item)) = joined {
            pool.remove(&item).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::{Mutex, Notify, Semaphore};

    /// Minimal FetchStack whose items are `u8` ids (also their own key).
    #[derive(Clone, Default)]
    struct TestStack {
        items: Arc<Mutex<Vec<u8>>>,
        added: Arc<Notify>,
    }
    impl TestStack {
        async fn add(&self, n: u8) {
            self.items.lock().await.push(n);
            self.added.notify_one();
        }
        async fn len(&self) -> usize {
            self.items.lock().await.len()
        }
    }
    #[async_trait::async_trait]
    impl FetchStack for TestStack {
        type Item = u8;
        type Key = u8;
        fn key(item: &u8) -> u8 {
            *item
        }
        async fn is_empty(&self) -> bool {
            self.items.lock().await.is_empty()
        }
        async fn next_untried(&self, tried: &HashSet<u8>) -> Option<u8> {
            self.items
                .lock()
                .await
                .iter()
                .rev()
                .find(|n| !tried.contains(*n))
                .copied()
        }
        async fn remove(&self, item: &u8) {
            self.items.lock().await.retain(|n| n != item);
        }
        async fn wait_for_add(&self) {
            self.added.notified().await;
        }
    }

    fn config() -> FetchConfig {
        FetchConfig {
            concurrency: 2,
            attempt_timeout: Duration::from_secs(5),
            pass_interval: Duration::from_secs(60),
        }
    }

    async fn settle() {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn empty_pool_parks_until_an_item_is_added() {
        let pool = TestStack::default();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(n);
                    true
                }
            }))
        };
        settle().await;
        assert!(calls.lock().await.is_empty());
        pool.add(1).await;
        settle().await;
        assert_eq!(calls.lock().await.as_slice(), &[1]);
        assert!(pool.is_empty().await);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn one_pass_drains_all_succeeding_items() {
        let pool = TestStack::default();
        for n in 1..=3 {
            pool.add(n).await;
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(n);
                    true
                }
            }))
        };
        settle().await;
        assert!(pool.is_empty().await);
        let fetched: HashSet<u8> = calls.lock().await.iter().copied().collect();
        assert_eq!(fetched, HashSet::from([1, 2, 3]));
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn failing_item_is_retried_about_one_interval_later() {
        let pool = TestStack::default();
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();
        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_n, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push(start.elapsed());
                    false
                }
            }))
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
        pool.add(1).await;
        tokio::time::sleep(Duration::from_secs(61)).await;
        let times = times.lock().await.clone();
        assert_eq!(times.len(), 2);
        let gap = times[1] - times[0];
        assert!(
            gap >= Duration::from_secs(59) && gap <= Duration::from_secs(61),
            "expected ~60s between passes, got {gap:?}"
        );
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn adding_an_item_wakes_the_loop_before_the_interval() {
        let pool = TestStack::default();
        pool.add(1).await;
        let times = Arc::new(Mutex::new(Vec::new()));
        let start = tokio::time::Instant::now();
        let handle = {
            let times = times.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |n, _t| {
                let times = times.clone();
                async move {
                    times.lock().await.push((n, start.elapsed()));
                    false
                }
            }))
        };
        tokio::time::sleep(Duration::from_secs(5)).await;
        pool.add(2).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let times = times.lock().await.clone();
        let woke_early = times
            .iter()
            .any(|(n, at)| *n == 2 && *at < Duration::from_secs(30));
        assert!(woke_early, "expected item 2 fetched early, got {times:?}");
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn never_exceeds_the_concurrency_limit() {
        let pool = TestStack::default();
        for n in 1..=4 {
            pool.add(n).await;
        }
        let gate = Arc::new(Semaphore::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let handle = {
            let gate = gate.clone();
            let in_flight = in_flight.clone();
            let max_in_flight = max_in_flight.clone();
            tokio::spawn(fetch_loop(pool.clone(), config(), move |_n, _t| {
                let gate = gate.clone();
                let in_flight = in_flight.clone();
                let max_in_flight = max_in_flight.clone();
                async move {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    max_in_flight.fetch_max(now, Ordering::SeqCst);
                    gate.acquire().await.unwrap().forget();
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    true
                }
            }))
        };
        settle().await;
        assert_eq!(in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(pool.len().await, 4);
        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(pool.len().await, 2);
        gate.add_permits(2);
        settle().await;
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert!(pool.is_empty().await);
        handle.abort();
    }
}
```

These five tests are the node's existing loop tests (`crates/dashchat-node/src/blob_sync.rs`, the `tests` module fns `empty_pool_parks_until_a_hash_is_added`, `one_pass_drains_all_succeeding_items`, `failing_item_is_retried_about_one_interval_later`, `adding_a_hash_wakes_the_loop_before_the_interval`, `never_exceeds_the_concurrency_limit`) re-expressed against the generic `TestStack` — they are deleted from the node in Step 5. Add `mod fetch_loop;` and `pub use fetch_loop::{fetch_loop, FetchConfig, FetchStack};` to `crates/dashchat-utils/src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run -p dashchat-utils fetch_loop`
Expected: FAIL to COMPILE first (symbols undefined) — that counts as the failing state; once the code above compiles, the tests pass. (If you prefer a red-then-green within compilation, temporarily stub `fetch_loop` with `todo!()`, confirm the tests fail, then paste the real body.)

- [ ] **Step 3: Build dashchat-utils**

Run: `cargo build -p dashchat-utils`
Expected: PASS.

- [ ] **Step 4: Run the shared tests**

Run: `cargo nextest run -p dashchat-utils fetch_loop`
Expected: PASS (all five).

- [ ] **Step 5: Migrate the node to the shared loop**

In `crates/dashchat-node/Cargo.toml` ensure `dashchat-utils = { path = "../dashchat-utils" }` is a dependency (add if absent).

In `crates/dashchat-node/src/blob_sync.rs`:
1. Delete the local `BlobFetchConfig` struct + its `Default` impl, and the free fns `fetch_loop` and `run_fetch_pass` (the whole loop machinery now lives in `dashchat-utils`).
2. Delete the five loop unit tests listed in Step 1 from the `tests` module (they moved). Keep any pool-specific tests.
3. Add at the top: `pub use dashchat_utils::FetchConfig as BlobFetchConfig;` and `use dashchat_utils::FetchStack;`.
4. Implement `FetchStack` for the existing `BlobFetchPool` (`Item = (LogId, iroh_blobs::Hash)`, `Key = iroh_blobs::Hash`), delegating to its existing inherent methods:

```rust
#[async_trait::async_trait]
impl FetchStack for BlobFetchPool {
    type Item = (LogId, iroh_blobs::Hash);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.1
    }
    async fn is_empty(&self) -> bool {
        self.stack.lock().await.is_empty()
    }
    async fn next_untried(
        &self,
        tried: &std::collections::HashSet<iroh_blobs::Hash>,
    ) -> Option<Self::Item> {
        let stack = self.stack.lock().await;
        stack.iter().rev().find(|(_, hash)| !tried.contains(hash)).copied()
    }
    async fn remove(&self, item: &Self::Item) {
        self.stack.lock().await.retain(|entry| entry != item);
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}
```
(The pool's inherent `is_empty`/`next_untried`/`remove` may now be redundant — remove the ones nothing else calls; keep `add` and `from_ops`. `async-trait` is already a node dependency.)

5. Change `BlobSync::spawn_fetch_loop` to call the shared loop, destructuring the item:

```rust
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(dashchat_utils::fetch_loop(
            pool,
            config,
            move |(log_id, hash), attempt_timeout| {
                let this = this.clone();
                async move { this.try_fetch(log_id, hash, attempt_timeout).await }
            },
        ))
    }
```
`try_fetch` keeps its current `(LogId, Hash, Duration)` signature.

- [ ] **Step 6: Build + run the node's blob_sync tests and the existing integration test**

Run: `cargo nextest run -p dashchat-node blob_sync && cargo nextest run -p dashchat-node media_blob_syncs_between_nodes`
Expected: PASS. (The node behaves identically; the loop just lives elsewhere now.)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: extract generic fetch loop into dashchat-utils

Moves the blob fetch-loop control logic behind a FetchStack trait in
dashchat-utils and migrates dashchat-node onto it, so the mailbox server
can reuse the same loop without depending on dashchat-node's Payload.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task 5: Implement the mailbox's `hash → set<EndpointId>` pool on `FetchStack`

**Files:**
- Modify: `crates/mailbox-server/src/blob_sync.rs` (add `BlobFetchPool`, impl `FetchStack`, `try_fetch`, `spawn_fetch_loop`; store the pool on `BlobSync`)
- Modify: `crates/mailbox-server/Cargo.toml` (add `dashchat-utils`, `async-trait`)
- Modify: `crates/mailbox-server/src/lib.rs` (spawn the loop in `spawn_server`, export the pool)

**Interfaces:**
- Consumes: `BlobSync` (Task 3); `dashchat_utils::{FetchConfig, FetchStack, fetch_loop}` (Task 4).
- Produces:
  - `#[derive(Clone, Default)] pub struct BlobFetchPool` with inherent `pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId)`, `pub(crate) async fn is_empty(&self) -> bool`, `pub(crate) async fn next_untried(&self, tried: &HashSet<iroh_blobs::Hash>) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)>`, `pub(crate) async fn remove(&self, hash: iroh_blobs::Hash)`.
  - `impl FetchStack for BlobFetchPool` (`Item = (iroh_blobs::Hash, Vec<iroh::EndpointId>)`, `Key = iroh_blobs::Hash`).
  - `BlobSync.fetch_pool: BlobFetchPool` + `pub fn fetch_pool(&self) -> &BlobFetchPool`.
  - `pub fn BlobSync::spawn_fetch_loop(&self, config: FetchConfig) -> tokio::task::JoinHandle<()>`.

- [ ] **Step 1: Add deps and write the failing pool test**

In `crates/mailbox-server/Cargo.toml` `[dependencies]` add `dashchat-utils = { path = "../dashchat-utils" }` and `async-trait = "0.1"` (match the workspace version). Append to `crates/mailbox-server/src/blob_sync.rs`'s `tests` module:

```rust
    use std::collections::HashSet;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }
    fn endpoint_id(n: u8) -> iroh::EndpointId {
        iroh::SecretKey::from_bytes(&[n; 32]).public()
    }

    #[tokio::test]
    async fn pool_dedupes_sources_per_hash() {
        let pool = BlobFetchPool::default();
        pool.add_source(hash(1), endpoint_id(10)).await;
        pool.add_source(hash(1), endpoint_id(11)).await;
        pool.add_source(hash(1), endpoint_id(10)).await; // dup source
        let tried = HashSet::new();
        let (h, sources) = pool.next_untried(&tried).await.unwrap();
        assert_eq!(h, hash(1));
        assert_eq!(sources.len(), 2);
    }

    #[tokio::test]
    async fn pool_remove_drops_the_hash() {
        let pool = BlobFetchPool::default();
        pool.add_source(hash(1), endpoint_id(1)).await;
        pool.remove(hash(1)).await;
        assert!(pool.is_empty().await);
    }
```

NOTE: if `iroh::SecretKey::from_bytes` / `.public()` differ for this iroh version, adjust `endpoint_id(n)` to yield distinct deterministic `EndpointId`s. The loop behavior itself is already covered by `dashchat-utils` (Task 4) — do not re-test it here.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: FAIL (pool symbols undefined).

- [ ] **Step 3: Implement the pool, its `FetchStack` impl, and `try_fetch` / `spawn_fetch_loop`**

Add to `crates/mailbox-server/src/blob_sync.rs`. Use `BTreeMap<Hash, BTreeSet<EndpointId>>` for deterministic iteration:

```rust
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use std::time::Duration;

use dashchat_utils::{fetch_loop, FetchConfig, FetchStack};
use iroh_blobs::api::downloader::Shuffled;
use iroh_blobs::protocol::GetRequest;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    sources: Arc<Mutex<BTreeMap<iroh_blobs::Hash, BTreeSet<iroh::EndpointId>>>>,
    added: Arc<Notify>,
}

impl BlobFetchPool {
    pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId) {
        self.sources.lock().await.entry(hash).or_default().insert(source);
        self.added.notify_one();
    }
    pub(crate) async fn is_empty(&self) -> bool {
        self.sources.lock().await.is_empty()
    }
    pub(crate) async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)> {
        let map = self.sources.lock().await;
        map.iter()
            .find(|(hash, _)| !tried.contains(*hash))
            .map(|(hash, sources)| (*hash, sources.iter().copied().collect()))
    }
    pub(crate) async fn remove(&self, hash: iroh_blobs::Hash) {
        self.sources.lock().await.remove(&hash);
    }
}

#[async_trait::async_trait]
impl FetchStack for BlobFetchPool {
    type Item = (iroh_blobs::Hash, Vec<iroh::EndpointId>);
    type Key = iroh_blobs::Hash;

    fn key(item: &Self::Item) -> Self::Key {
        item.0
    }
    async fn is_empty(&self) -> bool {
        BlobFetchPool::is_empty(self).await
    }
    async fn next_untried(&self, tried: &HashSet<iroh_blobs::Hash>) -> Option<Self::Item> {
        BlobFetchPool::next_untried(self, tried).await
    }
    async fn remove(&self, item: &Self::Item) {
        BlobFetchPool::remove(self, item.0).await;
    }
    async fn wait_for_add(&self) {
        self.added.notified().await;
    }
}
```

Add `fetch_pool: BlobFetchPool` to the `BlobSync` struct, initialize it (`fetch_pool: BlobFetchPool::default()`) in `BlobSync::new`, and add `pub fn fetch_pool(&self) -> &BlobFetchPool { &self.fetch_pool }`. Add the spawn + try_fetch methods:

```rust
impl BlobSync {
    pub fn spawn_fetch_loop(&self, config: FetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(fetch_loop(pool, config, move |(hash, sources), timeout| {
            let this = this.clone();
            async move { this.try_fetch(hash, sources, timeout).await }
        }))
    }

    async fn try_fetch(
        &self,
        hash: iroh_blobs::Hash,
        sources: Vec<iroh::EndpointId>,
        attempt_timeout: Duration,
    ) -> bool {
        if self.blobs.has(hash).await.unwrap_or(false) {
            return true;
        }
        if sources.is_empty() {
            return false;
        }
        let providers = Shuffled::new(sources.into_iter().map(Into::into).collect());
        match tokio::time::timeout(
            attempt_timeout,
            self.downloader.download(GetRequest::all(hash), providers),
        )
        .await
        {
            Ok(Ok(_)) => true,
            Ok(Err(err)) => {
                tracing::debug!(%hash, ?err, "mailbox blob download failed");
                false
            }
            Err(_) => {
                tracing::warn!(%hash, "mailbox blob download timed out");
                false
            }
        }
    }
}
```

Match `self.blobs.has(...)`, `Shuffled`, `GetRequest::all`, and `downloader.download` to the node's working usage in `crates/dashchat-node/src/blob_sync.rs` (the `try_fetch` body there) — same iroh-blobs version, identical calls.

- [ ] **Step 4: Run the pool tests**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: PASS (both pool tests).

- [ ] **Step 5: Spawn the fetch loop in `spawn_server`**

In `crates/mailbox-server/src/lib.rs::spawn_server`, after building `blob_sync`, spawn the loop and keep the handle so it is aborted on shutdown:

```rust
    let blob_fetch_handle = blob_sync.spawn_fetch_loop(dashchat_utils::FetchConfig::default());
```
At the existing graceful-shutdown tail (next to `cleanup_task.abort();`), add `blob_fetch_handle.abort();`. Export the pool: `pub use blob_sync::{BlobSync, BlobFetchPool};` (FetchConfig comes from `dashchat_utils`).

- [ ] **Step 6: Build + full server tests**

Run: `cargo nextest run -p mailbox-server`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(mailbox): hash->sources fetch pool on shared FetchStack loop

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 4 — Request API: clients announce hashes + source

### Task 6: Extend `StoreBlipsRequest` and record sources in the store handler

**Files:**
- Modify: `crates/mailbox-server/src/store_blips.rs` (add fields to `StoreBlipsRequest`, record sources after a successful store)
- Modify: `crates/mailbox-client/src/toy.rs` (send the new fields)

**Interfaces:**
- Consumes: `BlobFetchPool::add_source` (Task 5), `AppState.blob_sync` (Task 3).
- Produces: `StoreBlipsRequest { blips, blob_hashes: Vec<iroh_blobs::Hash>, sender_pubkey: iroh::EndpointId, signature: Vec<u8> }`.

- [ ] **Step 1: Write the failing test: storing records sources into the fetch pool**

Add a test in `crates/mailbox-server/src/store_blips.rs` (or a new `#[cfg(test)] mod tests`) that calls the store handler with a populated `blob_hashes` + `sender_pubkey` and asserts the pool now yields that hash with that source. Because the handler needs `AppState`, drive it through the inner recording function you will extract. Define the failing test against a helper `record_blob_sources(blob_sync: &BlobSync, hashes: &[iroh_blobs::Hash], source: iroh::EndpointId)`:

```rust
    #[tokio::test(start_paused = true)]
    async fn store_records_blob_sources() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate(&mut rand::rngs::OsRng);
        let blob_sync = crate::BlobSync::new(key, dir.path().to_path_buf()).await.unwrap();
        let source = iroh::SecretKey::from_bytes(&[7; 32]).public();
        let h = iroh_blobs::Hash::new([9; 32]);

        crate::store_blips::record_blob_sources(&blob_sync, &[h], source).await;

        let tried = std::collections::HashSet::new();
        let (got, sources) = blob_sync.fetch_pool_for_test().next_untried(&tried).await.unwrap();
        assert_eq!(got, h);
        assert!(sources.contains(&source));
    }
```

Add a test-only accessor on `BlobSync` (in `blob_sync.rs`): `#[cfg(test)] pub fn fetch_pool_for_test(&self) -> BlobFetchPool { self.fetch_pool.clone() }`, and make `BlobFetchPool::next_untried` / `is_empty` reachable from `store_blips` tests (mark them `pub(crate)`).

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p mailbox-server store_records_blob_sources`
Expected: FAIL (`record_blob_sources` undefined).

- [ ] **Step 3: Add fields to `StoreBlipsRequest` and implement `record_blob_sources`**

In `crates/mailbox-server/src/store_blips.rs`:

```rust
#[derive(Serialize, Deserialize)]
pub struct StoreBlipsRequest {
    pub blips: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blip>>>,
    #[serde(default)]
    pub blob_hashes: Vec<iroh_blobs::Hash>,
    pub sender_pubkey: iroh::EndpointId,
    #[serde(default)]
    pub signature: Vec<u8>,
}

pub async fn record_blob_sources(
    blob_sync: &crate::BlobSync,
    hashes: &[iroh_blobs::Hash],
    source: iroh::EndpointId,
) {
    for hash in hashes {
        blob_sync.fetch_pool().add_source(*hash, source).await;
    }
}
```

Add `pub fn fetch_pool(&self) -> &BlobFetchPool { &self.fetch_pool }` to `BlobSync` (and keep `add_source` public). Then in the `store_blips` handler, after the blocking store succeeds and before/after `notify_topics_subscribers`, record the sources:

```rust
    record_blob_sources(&state.blob_sync, &payload.blob_hashes, payload.sender_pubkey).await;
```
(`store_blips` already takes `State(state): State<AppState>`.)

- [ ] **Step 4: Run the test**

Run: `cargo nextest run -p mailbox-server store_records_blob_sources`
Expected: PASS.

- [ ] **Step 5: Make the toy client send the new fields**

In `crates/mailbox-client/src/toy.rs::publish`, the client builds a `StoreBlipsRequest`. The `MailboxClient` trait does not currently expose the node's own pubkey or per-op blob hashes, so thread them in:

- The toy client needs its own `EndpointId`. Add a field `sender_pubkey: iroh::EndpointId` to `ToyMailboxClient` and a constructor param. Update `ToyMailboxClient::new(id: MailboxId, base_url: impl Into<String>, sender_pubkey: iroh::EndpointId)`.
- The blob hashes come from the ops being published. `MailboxItem` exposes `hash()` (the op hash), not media blob hashes. Add a method to the `MailboxItem` trait: `fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> { Vec::new() }` (default empty), and override it in `dashchat-node`'s `MailboxOperation` impl to extract media hashes from the body. See Task 7.

For now in `publish`, collect hashes across the ops and set the fields:
```rust
        let blob_hashes: Vec<iroh_blobs::Hash> =
            ops.iter().flat_map(|op| op.blob_hashes()).collect();
        let request = StoreBlipsRequest {
            blips,
            blob_hashes,
            sender_pubkey: self.sender_pubkey,
            signature: Vec::new(),
        };
```
(Note `ops` is consumed by the existing loop; compute `blob_hashes` before the loop, or clone the needed data.)

- [ ] **Step 6: Build (add the `blob_hashes` trait default so callers compile)**

Run: `cargo build -p mailbox-server -p mailbox-client`
Expected: may FAIL on the `blob_hashes()` trait method until the default exists. Add the default method now to `MailboxItem` in `crates/mailbox-client/src/lib.rs` (the `MailboxOperation` override lands in Task 7):
```rust
    fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
        Vec::new()
    }
```
Add `iroh-blobs` to `mailbox-client/Cargo.toml` deps (workspace version). Rebuild; expected PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(mailbox): clients announce blob hashes + sender pubkey on store

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 5 — Node-side: MailboxId = EndpointId, sources, blob hashes

### Task 7: `MailboxOperation::blob_hashes`, mailbox as a blob source, EndpointId as MailboxId

**Files:**
- Modify: `crates/dashchat-node/src/mailbox.rs` (impl `blob_hashes` on `MailboxOperation`)
- Modify: `crates/dashchat-node/src/blob_sync.rs` (un-comment + implement mailbox sources in `MixedSourceLookup::sources`)
- Modify: `crates/mailbox-client/src/manager.rs` (add `Mailboxes::get_sources`)
- Modify: callers of `ToyMailboxClient::new` — `src-tauri/src/mailbox.rs`, `src-tauri/src/setup.rs`, `crates/dashchat-node/tests/mailboxes.rs`
- Modify: `src-tauri/src/mailbox/server.rs` + `src-tauri/src/mailbox.rs` (mDNS: announce + consume the mailbox EndpointId as the MailboxId — see Open Risk)

**Interfaces:**
- Consumes: `BlobFetchPool`/sources concepts; the node's `DeviceId == EndpointId` fact.
- Produces:
  - `MailboxOperation::blob_hashes(&self) -> Vec<iroh_blobs::Hash>`.
  - `Mailboxes::get_sources(&self, topic) -> anyhow::Result<Vec<iroh::EndpointId>>` returning the EndpointIds of mailboxes tracking `topic`.
  - `MailboxId` carrying the mailbox's iroh EndpointId hex.

- [ ] **Step 1: Write the failing test for `MailboxOperation::blob_hashes`**

In `crates/dashchat-node/src/mailbox.rs` tests module, add a test that a `MailboxOperation` whose body is a chat message with a photo attachment returns the media hash from `blob_hashes()`. Use the existing helpers (`MediaAttachment::Photos`, the message-building path used in `tests/blob_sync.rs`). Sketch:

```rust
    #[tokio::test(flavor = "multi_thread")]
    async fn mailbox_operation_exposes_media_blob_hashes() {
        // Build a MailboxOperation carrying a chat message with one photo,
        // then assert blob_hashes() returns exactly that photo's iroh hash.
        // Reuse the message construction path from chat/message.rs; the hash
        // equals the MediaMetaItem.hash stored on the message body.
        // (Construct via the same code send_message uses to compute media meta.)
    }
```
Fill this in concretely using the message/media API in `crates/dashchat-node/src/chat/message.rs` (`MediaMetaItem.hash: iroh_blobs::Hash`, `Payload::Chat(ChatPayload::Message(m))`, `m.media_meta()`), matching how `BlobFetchPool::from_ops` (`crates/dashchat-node/src/blob_sync.rs:246-270`) already extracts hashes — that loop is the reference implementation for the body→hashes extraction.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p dashchat-node mailbox_operation_exposes_media_blob_hashes`
Expected: FAIL (default `blob_hashes()` returns empty).

- [ ] **Step 3: Implement `blob_hashes` on `MailboxOperation`**

In `crates/dashchat-node/src/mailbox.rs`, in the `impl MailboxItem for MailboxOperation` block, override:

```rust
    fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
        let Some(body) = &self.body else {
            return Vec::new();
        };
        let Ok(payload) = crate::Payload::try_from_body(body) else {
            return Vec::new();
        };
        match payload {
            crate::Payload::Chat(crate::ChatPayload::Message(m)) => m
                .media_meta()
                .map(|items| items.iter().map(|item| item.hash).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
```
Match `try_from_body`, `Payload`, `ChatPayload`, and `media_meta()` to their exact paths/signatures (the `from_ops` loop uses `Payload::try_from_body(&body)` and `m.media_meta()`).

- [ ] **Step 4: Run the test**

Run: `cargo nextest run -p dashchat-node mailbox_operation_exposes_media_blob_hashes`
Expected: PASS.

- [ ] **Step 5: Implement `Mailboxes::get_sources` and un-comment the source lookup**

In `crates/mailbox-client/src/manager.rs`, add to `impl Mailboxes`:

```rust
    /// EndpointIds of mailboxes currently tracking `topic`, usable as blob sources.
    pub async fn get_sources(&self, topic: &Item::Topic) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let topics = self.topics.lock().await;
        if !topics.contains_key(topic) {
            return Ok(Vec::new());
        }
        drop(topics);
        let ids = self.mailboxes.lock().await.keys().cloned().collect::<Vec<MailboxId>>();
        ids.into_iter()
            .map(|id| parse_endpoint_id(&id))
            .collect::<anyhow::Result<Vec<_>>>()
    }
```
For `parse_endpoint_id`, reuse the canonical `MailboxId` codec from Task 3: `mailbox_server::decode_mailbox_id(id)` (base64url no-pad → `EndpointId`). Add `mailbox-server` as a (non-dev) dependency of `mailbox-client` if not already linked, OR — to avoid that dependency direction — move `encode_mailbox_id`/`decode_mailbox_id` into `mailbox-client/src/lib.rs` (where `MailboxId` is defined) and have `mailbox-server` re-export them from there. **Prefer the latter**: the codec belongs with the `MailboxId` type in `mailbox-client`. Update Task 3's `encode_mailbox_id` call in `health_check` to `mailbox_client::encode_mailbox_id(...)` accordingly. `iroh` is already a dep of `mailbox-client` (from Task 6).

In `crates/dashchat-node/src/blob_sync.rs::MixedSourceLookup::sources`, change the signature to take `&Item::Topic`-equivalent and un-comment:
```rust
        sources.extend(self.mailboxes.get_sources(&log_id_to_topic(log_id)).await?);
```
NOTE: `sources()` currently takes a `LogId`; `get_sources` wants `Item::Topic` (= `TopicId`). Convert `LogId → TopicId` using whatever existing accessor maps them (a `LogId` is `LogId::from_topic(TopicId)`; find the inverse — likely `log_id.topic()` or a field). If no direct inverse exists, change `MixedSourceLookup::sources` callers to pass the `TopicId` they already have. Verify against `crate::topic`.

- [ ] **Step 6: Update all `ToyMailboxClient::new` callers to pass a `sender_pubkey`**

For node/tauri callers, the node's own `EndpointId` = `EndpointId::from_bytes(device_id.as_bytes())`. In `src-tauri/src/mailbox.rs` (mDNS register) and `src-tauri/src/setup.rs` (cloud), obtain the node's device id and convert. Add a small accessor if needed on `Node`: `pub fn endpoint_id(&self) -> iroh::EndpointId { iroh::EndpointId::from_bytes(self.device_id().as_bytes()).expect("device id is a valid endpoint id") }` (in `crates/dashchat-node/src/node.rs`). Pass `node.endpoint_id()` as the third arg to `ToyMailboxClient::new`.

In `crates/dashchat-node/tests/mailboxes.rs`, pass the test node's `endpoint_id()` similarly.

- [ ] **Step 7: Make the MailboxId equal the mailbox's EndpointId (mDNS + cloud)**

- Cloud: wherever `ToyMailboxClient::new(PRODUCTION_MAILBOX_ID, PRODUCTION_MAILBOX_URL, ...)` is built (`src-tauri/src/setup.rs`), fetch the mailbox's EndpointId from its `/health` endpoint (now includes `endpoint_id`) and use that hex string as the `MailboxId` instead of the hardcoded `PRODUCTION_MAILBOX_ID`. Add a small `reqwest` GET to `/health` during registration.
- Local/mDNS: in `src-tauri/src/mailbox/server.rs::mdns_service_info`, set the instance name to the mailbox server's `MailboxId` — i.e. `encode_mailbox_id(endpoint_id)` (base64url, 43 chars, fits the single DNS label). The local mailbox server must expose its EndpointId to the tauri layer — read it from the spawned server (return it from `start_local_mailbox`) or query `/health`. In `src-tauri/src/mailbox.rs::handle_browse_events`, the `mailbox_id` derived from the fullname is then already the canonical `MailboxId`, so no extra decoding is needed at registration; `ToyMailboxClient::new` receives it as-is.

This step is the largest behavioral change in Phase 5.

- [ ] **Step 8: Build everything**

Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 9: Run node + client + mailbox tests**

Run: `cargo nextest run -p dashchat-node -p mailbox-client -p mailbox-server`
Expected: PASS (existing tests, now with the new wiring).

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(mailbox): MailboxId=EndpointId, mailbox as blob source, op blob hashes

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 6 — End-to-end integration test

### Task 8: Staged-online mailbox blob relay test

**Files:**
- Create: `crates/dashchat-node/tests/mailbox_blob_sync.rs`

**Interfaces:**
- Consumes: everything above. Uses `mailbox_server::spawn_server` over a real TCP port + `ToyMailboxClient`, the `TestNode` helpers, `MediaAttachment::Photos`, `load_media`.

- [ ] **Step 1: Write the integration test (staged-online model)**

Create `crates/dashchat-node/tests/mailbox_blob_sync.rs`. Model the body on `tests/blob_sync.rs` but: (a) use a real `ToyMailboxClient` against a spawned `mailbox_server`, (b) bring Alice up, send media, wait for the mailbox to hold the blob, (c) drop Alice, (d) bring Bobbi up and assert he loads the blob (only possible via the mailbox).

```rust
use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::toy::ToyMailboxClient;
use p2panda::network::MdnsDiscoveryMode;

#[tokio::test(flavor = "multi_thread")]
async fn media_blob_relays_through_mailbox_when_sender_offline() {
    dashchat_node::testing::setup_tracing(&["dashchat=info", "mailbox_server=info"], true);

    // 1. Spawn a real mailbox server on an ephemeral port.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("mailbox.redb");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let addr = format!("127.0.0.1:{port}");
    let base_url = format!("http://{addr}");
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_db = db_path.clone();
    let server_addr = addr.clone();
    let server = tokio::spawn(async move {
        let _ = mailbox_server::spawn_server(server_db, server_addr, None, async move {
            let _ = stop_rx.await;
        })
        .await;
    });

    // Wait for /health and read the mailbox EndpointId.
    let mailbox_id = wait_for_mailbox_id(&base_url).await;

    // 2. mDNS on so the mailbox can discover node addresses.
    let mut config = NodeConfig::testing();
    config.mdns_mode = MdnsDiscoveryMode::Active;

    let photo_bytes: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let media = MediaAttachment::Photos {
        photos: vec![Photo {
            data: photo_bytes.clone(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
        }],
    };

    let poll = PollConfig::default();

    // Create Bobbi's identity up front (on a shared store path) so Alice can
    // derive the direct-chat topic to his agent id without him being online,
    // then drop him until step 6. See the design notes below.
    let bobbi_store = std::sync::Arc::new(tempfile::tempdir().unwrap());
    let bobbi0 = TestNode::new_at_path(config.clone(), "bobbi", bobbi_store.clone()).await;
    let bobbi_agent = bobbi0.agent_id();
    drop(bobbi0);

    // 3. Alice online: send media, publish to mailbox.
    let alice = TestNode::new(config.clone(), "alice").await;
    let alice_endpoint = alice.endpoint_id();
    let alice_client =
        ToyMailboxClient::new(mailbox_id.clone(), base_url.clone(), alice_endpoint);
    let alice = alice.add_mailbox_client(alice_client).await;

    let chat = alice.direct_chat_topic(bobbi_agent);
    alice.register_topic(chat).await.unwrap();
    let meta = {
        alice.send_message(chat, "look", Some(media)).await.unwrap();
        alice
            .get_messages(chat)
            .await
            .unwrap()
            .into_iter()
            .find_map(|m| m.content.media_meta().cloned())
            .expect("alice has media meta")
    };

    // 4. Wait until the mailbox has fetched the blob from Alice.
    poll.wait_for(|| async {
        mailbox_has_blob(&base_url, &meta).await
    })
    .await
    .unwrap();

    // 5. Alice goes offline.
    drop(alice);

    // 6. Bobbi online (same identity/store as bobbi0): syncs op + downloads
    //    blob from the mailbox only.
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_store.clone()).await;
    let bobbi_client =
        ToyMailboxClient::new(mailbox_id.clone(), base_url.clone(), bobbi.endpoint_id());
    let bobbi = bobbi.add_mailbox_client(bobbi_client).await;
    bobbi.register_topic(chat).await.unwrap();

    poll.wait_for(|| async {
        bobbi
            .load_media(meta.clone())
            .await
            .map(|_| ())
            .map_err(|e| format!("blob not downloaded yet: {e:?}"))
    })
    .await
    .unwrap();

    let loaded = bobbi.load_media(meta).await.unwrap();
    let MediaAttachment::Photos { photos } = loaded else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos[0].data, photo_bytes);

    let _ = stop_tx.send(());
    let _ = server.await;
}
```

IMPORTANT design notes for the implementer:
- The contact/topic setup: the `tests/blob_sync.rs` test runs an explicit `initiate_and_establish_contact` between two live nodes. Here the nodes are never online together, so you cannot run that handshake live. Instead, establish the chat topic and group membership for BOTH agents while only Alice is up — i.e. set up the topic via a path that does not require Bobbi to be online (replicate what the mailbox `tests/mailboxes.rs::test_mailbox_late_join` does: it circumvents the contact-adding system and uses `direct_chat_topic` + `register_topic`). Use `alice.direct_chat_topic(bobbi.agent_id())` — to know bobbi's agent id without bobbi being online, create the `bobbi` `TestNode` first (so its identity exists), capture `bobbi.agent_id()`, then `drop(bobbi)` and recreate it from the same store path before step 6 using `TestNode::new_at_path` with a shared `TempDir` so Bobbi keeps the same identity/store. Adjust the skeleton accordingly: create both nodes' identities up front, derive the topic, then control who is "online".
- `wait_for_mailbox_id(base_url)`: GET `{base_url}/health`, parse JSON `{ "status", "endpoint_id" }`, retry until 200. Returns `endpoint_id` as the `MailboxId`.
- `mailbox_has_blob(base_url, meta)`: the mailbox has no HTTP endpoint to query blob presence. Instead, assert indirectly: poll until Bobbi (step 6) can load it — OR add a tiny test-only `/blobs/has/{hash}` route under `#[cfg(feature = "test_utils")]` returning whether `blob_sync.blobs.has(hash)` is true. Prefer the latter for a crisp "blob reached the mailbox" assertion. If added, gate it and enable the feature in the test's `Cargo.toml` dev-deps (`mailbox-server = { path = "...", features = ["test_utils"] }`).

- [ ] **Step 2: Add `mailbox-server` and `tempfile` as dev-dependencies of `dashchat-node`**

In `crates/dashchat-node/Cargo.toml` `[dev-dependencies]`, ensure `mailbox-server = { path = "../mailbox-server", features = ["test_utils"] }` and `tempfile` are present. Run `cargo build -p dashchat-node --tests`.

- [ ] **Step 3: Run the integration test**

Run: `cargo nextest run -p dashchat-node media_blob_relays_through_mailbox_when_sender_offline --no-capture`
Expected: PASS. If it flakes on timing, raise `poll_timeout`; if Bobbi can reach Alice directly (proving the staging failed), verify Alice is fully dropped (the `drop(alice)` must shut her endpoint — confirm `TestNode`/`Node` drop tears down the p2panda node; if not, add an explicit shutdown).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test(mailbox): end-to-end media blob relay through mailbox

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

- [ ] **Step 5: Full workspace verification**

Run: `cargo nextest run --workspace`
Expected: PASS. Also run `cargo build --workspace` to confirm `src-tauri` compiles with all changes.

---

## Notes / Open Risk (carried from the design doc)

- **mDNS EndpointId carrier (Task 7, Step 7) — RESOLVED:** the `MailboxId` is the EndpointId encoded as URL-safe base64 no-pad (43 chars), which fits a single mDNS DNS label (63-byte limit). So it goes straight in the mDNS instance name — no TXT property needed. The cloud case reads the same encoding from `/health`. The canonical codec (`encode_mailbox_id`/`decode_mailbox_id`) lives in `mailbox-client` alongside the `MailboxId` type.
- **iroh API surface:** several calls (`Endpoint::builder().secret_key().discovery_n0().bind()`, `endpoint.id()`/`.node_id()`, `SecretKey::generate`/`from_bytes`/`public`, `EndpointId::from_str`, `accept_unmixed` vs. a `Router`) must be confirmed against iroh `1.0.0-rc.1` / iroh-blobs `0.102.0`. The node's working `crates/dashchat-node/src/blob_sync.rs` is the authoritative reference for the iroh-blobs download/serve calls.
