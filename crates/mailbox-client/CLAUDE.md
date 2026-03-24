# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`mailbox-client` is a Rust crate that provides the client-side logic for syncing encrypted operation logs with remote mailbox servers. It is generic over the item type (`MailboxItem` trait) and handles bidirectional sync: fetching items the client is missing, and publishing items the server is missing.

## Commands

```bash
# Run tests (from workspace root)
cargo test -p mailbox-client

# Run a single test
cargo test -p mailbox-client -- test_name
```

## Architecture

**Core traits** (all in `lib.rs`):
- `MailboxItem` — defines the shape of a syncable item (has `hash`, `author`, `topic`, `seq_num`). Items are organized into logs keyed by `(topic, author)`.
- `LogStore` — `publish()` and `fetch()` operations against a remote mailbox. `fetch()` uses a subtractive filter: callers send their known heights per author, and the server returns everything *above* those heights plus a list of items it's *missing*.
- `RemoteBlobStore` — separate blob storage (fetch/publish by hash).
- `MailboxClient` — combines `LogStore` + `RemoteBlobStore` + an ID.
- `MailboxStore` — the local store interface (`get_log`, `get_log_heights`), implemented by the app's persistence layer.

**Mailbox manager** (`manager.rs`):
- `Mailboxes<Item, Store>` — manages multiple registered mailbox servers, polling them on independent schedules.
- Spawns a background tokio task that round-robins across registered mailboxes using a priority queue based on `next_poll` time.
- Each mailbox has a `SyncStatus` (Active → Degraded → Stopped) with exponential backoff on consecutive errors.
- Topic subscriptions control which topics get synced. Items received are forwarded via `mpsc::Sender<Item>` channels returned from `subscribe()`.
- `trigger_sync()` wakes the poll loop immediately (used after registering a new mailbox).

**Implementations**:
- `ToyMailboxClient` (`toy.rs`) — HTTP client for the toy mailbox server (`mailbox-server` crate). Encodes topics/authors as hex strings, serializes items as CBOR via p2panda-core. Uses endpoints `/dollops/store`, `/dollops/get`, `/blobs/store`, `/blobs/get`.
- `MemMailboxClient` (`mem.rs`) + `MemMailbox` (`mem_server.rs`) — in-memory implementation for testing. `MemMailbox` is the server state; `.client()` creates a client that shares state via `Arc<RwLock<...>>`.

**Testing** (`testing.rs`):
- `Msg` — minimal `MailboxItem` impl using `u8` topics, `char` authors, `u64` seq nums.
- `DummyStore` — no-op `MailboxStore` for manager tests.

## Key Design Details

- The `fetch()` protocol is bidirectional: the response includes both new items *and* a `missing` map telling the caller which items the server doesn't have, so the caller can `publish()` them back.
- Sequence numbers are 0-based and contiguous per `(topic, author)` log.
- The `named-id` feature gates human-readable renaming in trace logs (enabled by default).
- `HTTP_CLIENT` is a global lazy `reqwest::Client` with 5s connect / 10s total timeouts.
- The `mailbox-api` sibling crate defines shared request/response types and the `Opaq`/`OpaqHash` blob primitives.
