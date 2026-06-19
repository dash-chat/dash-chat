# Blob image loading + retry

**Date:** 2026-06-19
**Status:** Approved design

## Problem

Media images render from the `irohblob://{hash}` URI scheme, whose handler reads
the blob from the node's local store. When a blob has not yet been downloaded,
`load_blob` fails immediately, the handler returns 404, and the `<img>` shows a
broken image with no way to recover — even though the background fetch loop may
land the blob seconds later. There is also no loading state during the fetch.

We want:

1. The blob handler to optionally wait up to ~10s for a blob to arrive before
   giving up, so transient "not downloaded yet" cases self-heal while the user
   is looking at the chat.
2. A loading state while that fetch is in flight.
3. A way to retry a failed image by tapping it, that works reliably across app
   restarts (no stale-cache trap).

## Non-goals

- No automatic frontend retry loop. A failed image shows a tappable retry
  placeholder and stays there until the user taps. Persistent retrying of the
  *download* is already the backend fetch loop's job
  (`crates/dashchat-node/src/blob_sync.rs`) and is unchanged.
- No blurhash / progressive placeholder. The loading placeholder is a neutral
  box with a spinner.

## Design

### Backend: optional wait in `load_blob`

`crates/dashchat-node/src/node.rs` — change the signature so callers opt into
waiting:

```rust
pub async fn load_blob(
    &self,
    hash: &str,
    timeout: Option<Duration>,
) -> anyhow::Result<Vec<u8>> {
    let hash: iroh_blobs::Hash = hash.parse()?;
    match timeout {
        None => Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec()),
        Some(timeout) => {
            let deadline = Instant::now() + timeout;
            loop {
                if self.blob_sync.blobs.has(hash).await.unwrap_or(false) {
                    return Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec());
                }
                if Instant::now() >= deadline {
                    anyhow::bail!("blob {hash} not available after timeout");
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    }
}
```

- `None` preserves today's exact one-shot behavior.
- `Some(d)` polls `has()` every 250ms until the blob is present or the deadline
  passes. Passive polling (not active enqueue) because the handler has no
  `topic`, and `BlobFetchPool::from_ops` already enqueued the message's media,
  so the background loop is already trying to download it. Polling just gives
  that loop a window.

Only caller is the blob handler; it passes `Some(Duration::from_secs(10))`.

### Backend: don't cache failures

`src-tauri/src/blob_protocol.rs` — the 404 branch must not be cached, or a
failed attempt's URL can be served stale from the webview's persistent cache
after a restart. Add `Cache-Control: no-store` to the 404 response (the 200
response stays cacheable — content-addressed blobs are immutable). Pass
`Some(Duration::from_secs(10))` to `load_blob`.

### Frontend: `BlobImage.svelte` (shared)

New component `ui/src/lib/components/BlobImage.svelte`. Renders one blob-backed
image with three states.

Props:

```ts
interface Props {
    item: Photo | FileAttachment;
    alt: string;
    objectFit?: 'cover' | 'contain'; // default 'cover'
}
```

State:

```ts
let status = $state<'loading' | 'loaded' | 'error'>('loading');
let buster = $state(0); // 0 keeps a clean, cacheable URL on first load
const src = $derived(
    buster === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${buster}`,
);
function retry() {
    status = 'loading';
    buster = Date.now();
}
```

States:

- **loading** — request in flight (up to the backend's 10s wait). A neutral
  placeholder box with a small spinner; wrapper carries `aria-busy="true"`. The
  `<img>` is present (so `onload`/`onerror` fire) but visually covered until
  loaded.
- **loaded** (`onload`) — the real `<img src alt>` shows; placeholder removed.
- **error** (`onerror`) — replace with a focusable `<button>` retry placeholder
  with `aria-label` describing the failure and the alt/name (e.g. "Image failed
  to load — tap to retry"). `onclick={retry}`.

Cache-busting rationale: an `<img>` will not re-request a URL whose `src` is
unchanged, so retry must change the URL. A monotonic `0,1,2…` counter resets on
restart and would re-hit any cached failure; `Date.now()` is monotonic across
restarts so every retry, in every session, is a fresh URL. Combined with
`no-store` on failures, the cache can never strand an image. Initial render
(`buster === 0`) is query-free so a successful 200 caches cleanly and reopening
a chat loads instantly.

Accessibility: the success path keeps a real `<img>` with `alt` — no
regression. The only state without an `<img>` is `error`, where a labeled retry
button is strictly more accessible than a bare broken image.

Styling boundary: a consumer's scoped `.x img` rule will not reach an `<img>`
inside `BlobImage`. So `BlobImage` owns its inner img fill behavior (`width`/
`height: 100%`, `object-fit` from the prop) and its own `position: relative`
root for the absolutely-positioned (`inset: 0`) overlay. The consumer owns only
the *outer* sizing/layout of the `BlobImage` root.

### Consumers

- **PhotosAttachment** (`ui/src/lib/components/messages/attachments/PhotosAttachment.svelte`):
  replace the `<img src={mediaSrc(photo)} …>` at line 37 with
  `<BlobImage item={photo} alt={photo.name} objectFit="cover" />`. The `+N`
  overlay stays in the parent. The lone-image `min/max` width/height
  constraints (currently `.photo-cell:only-child img`) move onto the
  `.photo-cell`/BlobImage root, since they can no longer target the inner img.
- **Lightbox** (`ui/src/lib/components/messages/Lightbox.svelte`): swap its
  full-size image for `<BlobImage … objectFit="contain" />` so the lightbox
  also shows loading/retry. (Confirm exact markup when implementing.)

## Testing

- **Rust** (`node.rs` tests): `load_blob(hash, None)` errors immediately for an
  absent blob; `load_blob(hash, Some(short))` errors after the deadline when the
  blob never arrives; returns the bytes when the blob is present (or lands
  during the wait).
- **E2E**: extend the existing photo-message spec to assert the loading
  placeholder then the loaded image. For the error→tap→retry path, add a
  `window.__test` hook (in `ui/tests/setup-utils.ts`) to drive a known-missing
  blob, then assert the retry button appears and that tapping it re-requests.
  Confirm feasibility against the current media spec when writing the plan.
- Add `BlobImage` to the review-checks visit-all-pages coverage only if it
  introduces a new route (it does not — it is a component), so no review-checks
  change is expected.

## Open implementation details (decide while planning)

- Exact Lightbox markup and whether its placeholder sizing differs from the grid
  cell's.
- Spinner component already used elsewhere vs. a small inline one.
