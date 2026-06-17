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
- Commit after each task.

---

## Phase 1 — Rename blob → blip (mailbox encrypted log items)

This phase is a pure rename with **no behavior change**. The existing test suite must stay green. It is mechanical but touches wire + disk names.

### Task 1: Rename `blob` → `blip` across mailbox crates and consumers

**Files (all under repo root):**
- Rename: `crates/mailbox-server/src/blob.rs` → `crates/mailbox-server/src/blip.rs`
- Rename: `crates/mailbox-server/src/blobs_table.rs` → `crates/mailbox-server/src/blips_table.rs`
- Rename: `crates/mailbox-server/src/store_blobs.rs` → `crates/mailbox-server/src/store_blips.rs`
- Rename: `crates/mailbox-server/src/get_blobs.rs` → `crates/mailbox-server/src/get_blips.rs`
- Modify: `crates/mailbox-server/src/lib.rs`, `crates/mailbox-server/src/watermark.rs`, `crates/mailbox-server/src/cleanup.rs`, `crates/mailbox-server/src/watermarks_table.rs`, `crates/mailbox-server/src/notify_topics_subscribers.rs`, `crates/mailbox-server/src/test_utils.rs`
- Modify tests: `crates/mailbox-server/tests/integration.rs`, `tests/cleanup.rs`, `tests/watermark.rs`, `tests/stress.rs`, `tests/push_integration.rs`
- Modify: `crates/mailbox-client/src/toy.rs`, `crates/mailbox-client/src/mem.rs` (only if it references server types — verify)
- Modify any `dashchat-node` / `src-tauri` files that import the renamed server types (find via grep).

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

- [ ] **Step 3: Apply the mechanical identifier rename across the mailbox crates and consumers**

Run this from the repo root. It rewrites identifiers, the redb table string, and the route paths in one pass. The ordering matters: do the longer/compound identifiers implicitly via the generic `Blob`/`blob` substitutions (Rust is case-sensitive, so `Blob`→`Blip` and `blob`→`blip` cover `BlobsKey`→`BlipsKey`, `blobs_by_topic`→`blips_by_topic`, etc.).

```bash
# Limit scope to the two mailbox crates plus known consumers.
FILES=$(grep -rIl --include='*.rs' -e 'blob' -e 'Blob' -e 'BLOB' \
  crates/mailbox-server crates/mailbox-client crates/dashchat-node src-tauri)

for f in $FILES; do
  sed -i \
    -e 's/Blob/Blip/g' \
    -e 's/blob/blip/g' \
    -e 's/BLOB/BLIP/g' \
    "$f"
done
```

This also renames the route strings (`"/blobs/store"`→`"/blips/store"`), the redb table name (`TableDefinition::new("blobs")`→`"blips"`), module declarations (`mod blob;`→`mod blip;`), and base64 helper module names. That is intended.

- [ ] **Step 4: Verify no `blob`/`Blob` tokens remain in the renamed scope**

Run:
```bash
grep -rIn --include='*.rs' -e 'blob' -e 'Blob' -e 'BLOB' \
  crates/mailbox-server crates/mailbox-client crates/dashchat-node src-tauri
```
Expected: NO output. If any remain, they are either (a) legitimately iroh-blob references you are about to add later (there should be none yet in this phase) or (b) a missed spot — fix by editing the file directly. At this phase the answer must be empty.

- [ ] **Step 5: Build**

Run: `cargo build -p mailbox-server -p mailbox-client -p dashchat-node`
Expected: PASS. If `src-tauri` references server types, also `cargo build -p dashchat` (the tauri crate) — fix any stragglers the sed missed (e.g. a doc comment that the grep flagged).

- [ ] **Step 6: Run the full affected test suite**

Run: `cargo nextest run -p mailbox-server -p mailbox-client -p dashchat-node`
Expected: PASS, identical to the Step 1 baseline (same number of tests, just renamed).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(mailbox): rename blob -> blip to free 'blob' for iroh

Renames the mailbox encrypted-log-item concept from 'blob' to 'blip'
across mailbox-server, mailbox-client and consumers, including HTTP
route paths, JSON field names, and the redb table name. No behavior
change. Frees 'blob' for the upcoming iroh-blobs sense.

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

Change `health_check` to read the id from state:
```rust
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        endpoint_id: state.blob_sync.endpoint_id().to_string(),
    })
}
```
(Recall `health_check` currently takes no args; add the `State` extractor and keep the `get(health_check)` route.)

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

## Phase 3 — Server fetch loop

### Task 4: Add the `Hash → set<EndpointId>` fetch pool and loop

**Files:**
- Modify: `crates/mailbox-server/src/blob_sync.rs` (add the pool, loop, `try_fetch`, `spawn_fetch_loop`; store the pool on `BlobSync`)

**Interfaces:**
- Consumes: `BlobSync` (Task 3).
- Produces:
  - `pub struct BlobFetchConfig { pub concurrency: usize, pub attempt_timeout: std::time::Duration, pub pass_interval: std::time::Duration }` with `Default`.
  - `#[derive(Clone, Default)] pub struct BlobFetchPool` with:
    - `pub async fn add_source(&self, hash: iroh_blobs::Hash, source: iroh::EndpointId)`
    - `async fn is_empty(&self) -> bool`
    - `async fn next_untried(&self, tried: &std::collections::HashSet<iroh_blobs::Hash>) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)>`
    - `async fn remove(&self, hash: iroh_blobs::Hash)`
  - `BlobSync.fetch_pool: BlobFetchPool` (public).
  - `pub fn BlobSync::spawn_fetch_loop(&self, config: BlobFetchConfig) -> tokio::task::JoinHandle<()>`

This mirrors `crates/dashchat-node/src/blob_sync.rs`, but the pool maps `hash → set of sources` (deduped per your design) instead of a `Vec<(LogId, Hash)>` stack.

- [ ] **Step 1: Write the failing unit tests for the pool + loop**

Append to `crates/mailbox-server/src/blob_sync.rs`'s `tests` module (add imports at top of the module):

```rust
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }

    fn endpoint_id(n: u8) -> iroh::EndpointId {
        iroh::SecretKey::from_bytes(&[n; 32]).public()
    }

    fn fetch_config() -> BlobFetchConfig {
        BlobFetchConfig {
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

    #[tokio::test(start_paused = true)]
    async fn empty_pool_parks_until_a_hash_is_added() {
        let pool = BlobFetchPool::default();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), fetch_config(), move |h, _srcs, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(h);
                    true
                }
            }))
        };
        settle().await;
        assert!(calls.lock().await.is_empty());
        pool.add_source(hash(1), endpoint_id(1)).await;
        settle().await;
        assert_eq!(calls.lock().await.as_slice(), &[hash(1)]);
        assert!(pool.is_empty().await);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn one_pass_drains_all_succeeding_items() {
        let pool = BlobFetchPool::default();
        for n in 1..=3 {
            pool.add_source(hash(n), endpoint_id(n)).await;
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handle = {
            let calls = calls.clone();
            tokio::spawn(fetch_loop(pool.clone(), fetch_config(), move |h, _s, _t| {
                let calls = calls.clone();
                async move {
                    calls.lock().await.push(h);
                    true
                }
            }))
        };
        settle().await;
        assert!(pool.is_empty().await);
        let fetched: HashSet<_> = calls.lock().await.iter().copied().collect();
        assert_eq!(fetched, HashSet::from([hash(1), hash(2), hash(3)]));
        handle.abort();
    }
```

NOTE: if `iroh::SecretKey::from_bytes` / `.public()` differ for this version, adjust `endpoint_id(n)` to produce a distinct deterministic `EndpointId`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: FAIL (pool/loop symbols undefined).

- [ ] **Step 3: Implement the config, pool, and loop**

Add to `crates/mailbox-server/src/blob_sync.rs` (top-level), modeled directly on the node's file. Use `BTreeMap<Hash, BTreeSet<EndpointId>>` so iteration order is deterministic:

```rust
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use iroh_blobs::api::downloader::Shuffled;
use iroh_blobs::protocol::GetRequest;
use tokio::sync::{Mutex, Notify};
use tokio::task::{JoinHandle, JoinSet};

#[derive(Clone, Debug)]
pub struct BlobFetchConfig {
    pub concurrency: usize,
    pub attempt_timeout: Duration,
    pub pass_interval: Duration,
}

impl Default for BlobFetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(30),
            pass_interval: Duration::from_secs(60),
        }
    }
}

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

    async fn is_empty(&self) -> bool {
        self.sources.lock().await.is_empty()
    }

    async fn next_untried(
        &self,
        tried: &HashSet<iroh_blobs::Hash>,
    ) -> Option<(iroh_blobs::Hash, Vec<iroh::EndpointId>)> {
        let map = self.sources.lock().await;
        map.iter()
            .find(|(hash, _)| !tried.contains(*hash))
            .map(|(hash, sources)| (*hash, sources.iter().copied().collect()))
    }

    async fn remove(&self, hash: iroh_blobs::Hash) {
        self.sources.lock().await.remove(&hash);
    }
}

async fn fetch_loop<F, Fut>(pool: BlobFetchPool, config: BlobFetchConfig, fetch: F)
where
    F: Fn(iroh_blobs::Hash, Vec<iroh::EndpointId>, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let concurrency = config.concurrency.max(1);
    loop {
        let pass_start = Instant::now();
        run_fetch_pass(&pool, concurrency, config.attempt_timeout, &fetch).await;

        if pool.is_empty().await {
            pool.added.notified().await;
            continue;
        }
        let elapsed = pass_start.elapsed();
        if elapsed < config.pass_interval {
            tokio::select! {
                _ = tokio::time::sleep(config.pass_interval - elapsed) => {}
                _ = pool.added.notified() => {}
            }
        }
    }
}

async fn run_fetch_pass<F, Fut>(
    pool: &BlobFetchPool,
    concurrency: usize,
    attempt_timeout: Duration,
    fetch: &F,
) where
    F: Fn(iroh_blobs::Hash, Vec<iroh::EndpointId>, Duration) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send + 'static,
{
    let mut tried: HashSet<iroh_blobs::Hash> = HashSet::new();
    let mut in_flight: JoinSet<Option<iroh_blobs::Hash>> = JoinSet::new();
    loop {
        while in_flight.len() < concurrency {
            let Some((hash, sources)) = pool.next_untried(&tried).await else {
                break;
            };
            tried.insert(hash);
            let fetch = fetch.clone();
            in_flight.spawn(async move {
                fetch(hash, sources, attempt_timeout).await.then_some(hash)
            });
        }
        let Some(joined) = in_flight.join_next().await else {
            break;
        };
        if let Ok(Some(hash)) = joined {
            pool.remove(hash).await;
        }
    }
}
```

Add `fetch_pool: BlobFetchPool` to the `BlobSync` struct and initialize it (`fetch_pool: BlobFetchPool::default()`) in `BlobSync::new`. Add the spawn + try_fetch methods:

```rust
impl BlobSync {
    pub fn spawn_fetch_loop(&self, config: BlobFetchConfig) -> JoinHandle<()> {
        let this = self.clone();
        let pool = self.fetch_pool.clone();
        tokio::spawn(fetch_loop(pool, config, move |hash, sources, timeout| {
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

Match `self.blobs.has(...)`, `Shuffled`, `GetRequest::all`, and `downloader.download` to the node's working usage in `crates/dashchat-node/src/blob_sync.rs` (lines 96-136) — they are the same iroh-blobs version, so the calls should be identical.

- [ ] **Step 4: Run the tests**

Run: `cargo nextest run -p mailbox-server blob_sync`
Expected: PASS (all three loop tests + the dedupe test).

- [ ] **Step 5: Spawn the fetch loop in `spawn_server`**

In `crates/mailbox-server/src/lib.rs::spawn_server`, after building `blob_sync` and before/after `create_app`, spawn the loop and keep the handle so it is aborted on shutdown:

```rust
    let blob_fetch_handle = blob_sync.spawn_fetch_loop(BlobFetchConfig::default());
```
Add `use crate::blob_sync::BlobFetchConfig;` (or `pub use` it from lib). At the existing graceful-shutdown tail (next to `cleanup_task.abort();`), add `blob_fetch_handle.abort();`. Export the config: `pub use blob_sync::{BlobSync, BlobFetchConfig, BlobFetchPool};`.

- [ ] **Step 6: Build + full server tests**

Run: `cargo nextest run -p mailbox-server`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(mailbox): add hash->sources blob fetch loop

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 4 — Request API: clients announce hashes + source

### Task 5: Extend `StoreBlipsRequest` and record sources in the store handler

**Files:**
- Modify: `crates/mailbox-server/src/store_blips.rs` (add fields to `StoreBlipsRequest`, record sources after a successful store)
- Modify: `crates/mailbox-client/src/toy.rs` (send the new fields)

**Interfaces:**
- Consumes: `BlobFetchPool::add_source` (Task 4), `AppState.blob_sync` (Task 3).
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
- The blob hashes come from the ops being published. `MailboxItem` exposes `hash()` (the op hash), not media blob hashes. Add a method to the `MailboxItem` trait: `fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> { Vec::new() }` (default empty), and override it in `dashchat-node`'s `MailboxOperation` impl to extract media hashes from the body. See Task 6.

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

- [ ] **Step 6: Build (expect mem.rs / callers to break — fixed in Task 6)**

Run: `cargo build -p mailbox-server -p mailbox-client`
Expected: may FAIL on `blob_hashes()` trait method until Task 6 adds the default. If so, add the default method now to `MailboxItem` in `crates/mailbox-client/src/lib.rs`:
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

### Task 6: `MailboxOperation::blob_hashes`, mailbox as a blob source, EndpointId as MailboxId

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
Add a free helper `fn parse_endpoint_id(id: &MailboxId) -> anyhow::Result<iroh::EndpointId>` that parses the hex MailboxId (`id.parse::<iroh::EndpointId>()` — confirm the `FromStr`/`from_str` form for this iroh version; EndpointId is a 64-char hex string). Add `iroh` to `mailbox-client/Cargo.toml` if not already present (it now is, from Task 5).

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
- Local/mDNS: in `src-tauri/src/mailbox/server.rs::mdns_service_info`, set the instance name to the mailbox server's EndpointId hex (the local mailbox server must expose its EndpointId to the tauri layer — read it from the spawned server, e.g. return it from `start_local_mailbox` or query `/health`). In `src-tauri/src/mailbox.rs::handle_browse_events`, the `mailbox_id` derived from the fullname is then already the EndpointId. See Open Risk.

This step is the largest behavioral change in Phase 5; if the mDNS EndpointId carrier proves awkward within a single label, fall back to publishing the EndpointId in an mDNS TXT property and reading it in `ServiceResolved`.

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

### Task 7: Staged-online mailbox blob relay test

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

    let poll = PollConfig {
        poll_interval: Duration::from_millis(250),
        poll_timeout: Duration::from_secs(30),
    };

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

- **mDNS EndpointId carrier (Task 6, Step 7):** switching `MailboxId` to the iroh EndpointId means the local mDNS announce side must publish it and the browse side consume it. The EndpointId is 64 hex chars — fits a single DNS label (63-byte limit is tight; 64 chars EXCEEDS it). **Therefore prefer an mDNS TXT property** (e.g. `endpoint_id=<hex>`) over the instance name for the local case, and read it in `ServiceResolved`. Resolve the exact mechanism here; the cloud case uses `/health`.
- **iroh API surface:** several calls (`Endpoint::builder().secret_key().discovery_n0().bind()`, `endpoint.id()`/`.node_id()`, `SecretKey::generate`/`from_bytes`/`public`, `EndpointId::from_str`, `accept_unmixed` vs. a `Router`) must be confirmed against iroh `1.0.0-rc.1` / iroh-blobs `0.102.0`. The node's working `crates/dashchat-node/src/blob_sync.rs` is the authoritative reference for the iroh-blobs download/serve calls.
