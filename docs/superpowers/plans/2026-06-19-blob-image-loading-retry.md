# Blob Image Loading + Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give blob-backed images a loading state and a tap-to-retry path, backed by an optional bounded wait in the blob URI handler so transient "not downloaded yet" media self-heals.

**Architecture:** `Node::load_blob` gains an `Option<Duration>` timeout; `Some(d)` polls the local blob store every 250ms until the blob lands or the deadline passes (the background fetch loop is already trying to download it). The `irohblob://` handler passes `Some(10s)` and marks 404s `Cache-Control: no-store`. A new shared `BlobImage.svelte` renders one blob image with loading/loaded/error states, retrying via a `Date.now()` cache-buster so retries work across restarts. `PhotosAttachment` and `Lightbox` consume it.

**Tech Stack:** Rust (dashchat-node, Tauri src-tauri), Svelte 5 + TypeScript, Konsta UI (`Preloader`), `@mdi/js` icons, Paraglide i18n, WebdriverIO E2E.

## Global Constraints

- RTL-aware: use logical CSS props / Tailwind `ms-/me-/ps-/pe-/start-/end-/inset-inline`, never `left/right`/`ml-`/`pl-` etc. `inset: 0` and `inset-0` are allowed (symmetric).
- Write very few comments; only doc-comments on functions or *why* notes for non-obvious code.
- Only edit English source strings in `ui/messages/en.json`; never touch other locale files.
- Match existing style; modify the minimum necessary lines.
- `Duration`/`Instant` in `crates/dashchat-node/src/node.rs` MUST be fully qualified as `std::time::Duration` / `std::time::Instant` because `chrono::Duration` is already imported there (line 16).
- Run CI commands inside `nix develop`.

---

### Task 1: Backend — `load_blob` optional bounded wait

**Files:**
- Modify: `crates/dashchat-node/src/node.rs:1152-1159` (the `load_blob` method)
- Test: `crates/dashchat-node/src/node.rs` (new `#[cfg(test)] mod blob_load_tests` at end of file)

**Interfaces:**
- Consumes: `self.blob_sync.blobs` (`iroh_blobs::BlobsProtocol`) with `.has(hash) -> anyhow::Result<bool>`, `.get_bytes(hash) -> Result<Bytes>`, `.add_bytes(Vec<u8>) -> Result<tag>` where `tag.hash: iroh_blobs::Hash`.
- Consumes: `crate::testing::TestNode::new(NodeConfig::testing(), name).await`, which derefs to `Node` (so `node.load_blob(...)`, `node.blobs()` work). `node.blobs()` returns a `BlobsProtocol` clone.
- Produces: `pub async fn load_blob(&self, hash: &str, timeout: Option<std::time::Duration>) -> anyhow::Result<Vec<u8>>`. Used by Task 2.

- [ ] **Step 1: Write the failing tests**

Append to `crates/dashchat-node/src/node.rs`:

```rust
#[cfg(test)]
mod blob_load_tests {
    use crate::NodeConfig;
    use crate::testing::TestNode;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_present_returns_bytes_without_timeout() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let tag = node.blobs().add_bytes(b"hello".to_vec()).await.unwrap();
        let hash = tag.hash.to_string();

        let got = node.load_blob(&hash, None).await.unwrap();
        assert_eq!(got, b"hello");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_missing_without_timeout_errors_immediately() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let missing = iroh_blobs::Hash::new(b"missing-without-timeout").to_string();

        let err = node.load_blob(&missing, None).await;
        assert!(err.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_missing_with_timeout_errors_after_deadline() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let missing = iroh_blobs::Hash::new(b"missing-with-timeout").to_string();

        let start = std::time::Instant::now();
        let err = node.load_blob(&missing, Some(Duration::from_millis(400))).await;
        assert!(err.is_err());
        assert!(start.elapsed() >= Duration::from_millis(400));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_blob_with_timeout_returns_blob_that_lands_mid_wait() {
        let node = TestNode::new(NodeConfig::testing(), "alice").await;
        let content = b"arrives-late".to_vec();
        let hash = iroh_blobs::Hash::new(&content);

        let blobs = node.blobs();
        let content2 = content.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            blobs.add_bytes(content2).await.unwrap();
        });

        let got = node
            .load_blob(&hash.to_string(), Some(Duration::from_secs(3)))
            .await
            .unwrap();
        assert_eq!(got, content);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `nix develop --command cargo nextest run -p dashchat-node blob_load_tests`
Expected: compile error — `load_blob` takes 1 arg, tests pass 2. (Compile failure counts as the red state.)

- [ ] **Step 3: Implement the optional wait**

Replace `crates/dashchat-node/src/node.rs:1152-1159` with:

```rust
    /// Load the raw bytes of a single blob by its hash from the local blob store.
    ///
    /// With `timeout: Some(d)` the call polls the local store until the blob is
    /// present or `d` elapses, giving the background fetch loop a window to land
    /// a blob that has not been downloaded yet. `None` reads once and errors
    /// immediately if the blob is absent.
    ///
    /// Used by the `irohblob://` URI scheme handler to serve media to the webview.
    pub async fn load_blob(
        &self,
        hash: &str,
        timeout: Option<std::time::Duration>,
    ) -> anyhow::Result<Vec<u8>> {
        let hash: iroh_blobs::Hash = hash.parse()?;
        let Some(timeout) = timeout else {
            return Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec());
        };
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.blob_sync.blobs.has(hash).await.unwrap_or(false) {
                return Ok(self.blob_sync.blobs.get_bytes(hash).await?.to_vec());
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!("blob {hash} not available after {timeout:?}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `nix develop --command cargo nextest run -p dashchat-node blob_load_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/dashchat-node/src/node.rs
git commit -m "feat(node): optional bounded wait in load_blob

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Backend — handler waits 10s and never caches failures

**Files:**
- Modify: `src-tauri/src/blob_protocol.rs:35-53`

**Interfaces:**
- Consumes: `node.load_blob(hash, timeout)` from Task 1 (`Option<std::time::Duration>`).
- Produces: no new symbols. Behavior: handler holds the request open up to 10s; 404 responses carry `Cache-Control: no-store`.

- [ ] **Step 1: Pass a 10s timeout from `load`**

In `src-tauri/src/blob_protocol.rs`, change the `load` helper (line 48-53) call site to request a wait:

```rust
async fn load<R: Runtime>(app: &tauri::AppHandle<R>, hash: &str) -> anyhow::Result<Vec<u8>> {
    let node = app
        .try_state::<Node>()
        .ok_or_else(|| anyhow::anyhow!("node not yet initialized"))?;
    node.load_blob(hash, Some(std::time::Duration::from_secs(10)))
        .await
}
```

- [ ] **Step 2: Mark the 404 response non-cacheable**

In the `Err` arm of `handle` (lines 35-42), add the `Cache-Control` header so a failed attempt is never served stale after a restart:

```rust
            Err(err) => {
                log::error!("failed to load blob {hash:?}: {err:?}");
                tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::NOT_FOUND)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Cache-Control", "no-store")
                    .body(Vec::new())
                    .expect("valid response")
            }
```

(The `Ok` arm is unchanged — content-addressed 200s stay cacheable.)

- [ ] **Step 3: Verify it compiles**

Run: `nix develop --command cargo check -p dash-chat`
Expected: compiles clean. (If the crate name differs, use `cargo check` from `src-tauri/`.)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/blob_protocol.rs
git commit -m "feat(blob): wait up to 10s for media; never cache failures

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Frontend — `BlobImage.svelte` shared component

**Files:**
- Create: `ui/src/lib/components/BlobImage.svelte`
- Modify: `ui/messages/en.json` (add one English source string)

**Interfaces:**
- Consumes: `mediaSrc(item: Photo | FileAttachment): string` from `$lib/utils/media` (line 239); `Preloader` from `konsta/svelte`; `mdiReload` from `@mdi/js`; `m` from `$lib/paraglide/messages.js`.
- Produces: component `BlobImage` with props `{ item: Photo | FileAttachment; alt: string; imgClass?: string; imgStyle?: string }`. Renders an `<img data-testid="blob-image">` in loading/loaded states (with `imgClass`/`imgStyle` forwarded), a `[data-testid="blob-image-loading"]` overlay while loading, and a `<button data-testid="blob-image-retry">` in the error state. Overlays use `position: absolute; inset: 0`, so **consumers must give the immediate parent a positioning context** (`position: relative`). Listens for a `test-blob-force-error` window event whose `detail` equals `alt` to drive the error state in tests.

- [ ] **Step 1: Add the English retry label**

In `ui/messages/en.json`, add after the `"closeLightbox"` line (line 158) a new key (keep the file's alphabetical-ish grouping near other media strings is not required; place it adjacent to media keys):

```json
	"imageLoadFailedRetry": "Image failed to load. Tap to retry.",
```

(Ensure the preceding line keeps its trailing comma and JSON stays valid.)

- [ ] **Step 2: Create the component**

Create `ui/src/lib/components/BlobImage.svelte`:

```svelte
<script lang="ts">
	import type { FileAttachment, Photo } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import { m } from '$lib/paraglide/messages.js';
	import { Preloader } from 'konsta/svelte';
	import { mdiReload } from '@mdi/js';

	interface Props {
		item: Photo | FileAttachment;
		alt: string;
		/** Forwarded to the inner <img> (e.g. object-fit / sizing classes). */
		imgClass?: string;
		/** Forwarded to the inner <img> (e.g. zoom transform-origin). */
		imgStyle?: string;
	}

	let { item, alt, imgClass = '', imgStyle = '' }: Props = $props();

	let status = $state<'loading' | 'loaded' | 'error'>('loading');
	// 0 keeps the first request query-free so the cached 200 is reused; a retry
	// uses Date.now() so every attempt is a fresh URL even across app restarts.
	let buster = $state(0);
	const src = $derived(
		buster === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${buster}`,
	);

	function retry() {
		status = 'loading';
		buster = Date.now();
	}

	$effect(() => {
		function onForceError(e: Event) {
			if ((e as CustomEvent<string>).detail === alt) status = 'error';
		}
		window.addEventListener('test-blob-force-error', onForceError);
		return () =>
			window.removeEventListener('test-blob-force-error', onForceError);
	});
</script>

{#if status === 'error'}
	<button
		type="button"
		class="blob-image-retry {imgClass}"
		style={imgStyle}
		aria-label={m.imageLoadFailedRetry()}
		data-testid="blob-image-retry"
		onclick={retry}
	>
		<svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
			<path fill="currentColor" d={mdiReload} />
		</svg>
	</button>
{:else}
	<img
		{src}
		{alt}
		class={imgClass}
		style={imgStyle}
		data-testid="blob-image"
		onload={() => (status = 'loaded')}
		onerror={() => (status = 'error')}
	/>
	{#if status === 'loading'}
		<span
			class="blob-image-loading"
			aria-busy="true"
			data-testid="blob-image-loading"
		>
			<Preloader class="w-6 h-6" />
		</span>
	{/if}
{/if}

<style>
	.blob-image-loading {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(128, 128, 128, 0.08);
		pointer-events: none;
	}

	.blob-image-retry {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 64px;
		min-height: 64px;
		border: none;
		padding: 0;
		background: rgba(128, 128, 128, 0.12);
		color: rgba(0, 0, 0, 0.5);
		cursor: pointer;
	}

	:global(.dark) .blob-image-retry {
		color: rgba(255, 255, 255, 0.6);
	}
</style>
```

- [ ] **Step 3: Type-check**

Run (from `ui/`): `nix develop --command pnpm check`
Expected: no new type errors referencing `BlobImage.svelte` or `en.json`.

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/BlobImage.svelte ui/messages/en.json
git commit -m "feat(ui): add BlobImage with loading and retry states

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Frontend — PhotosAttachment uses BlobImage

**Files:**
- Modify: `ui/src/lib/components/messages/attachments/PhotosAttachment.svelte:34-43` (markup) and `:173-178` (`.photo-cell img` rule) and `:81-89` (`.photo-cell:only-child img` rule)

**Interfaces:**
- Consumes: `BlobImage` (Task 3). The `.photo-cell` button is already `position: relative` (line 163), so it is the positioning context BlobImage's overlays need.
- Produces: no new symbols. The grid markup keeps `data-testid="message-attachment-photos"` and the per-cell `<button>`; the `<img>` is now rendered by `BlobImage` (still an `<img alt>` with `data-testid="blob-image"`, so `messages.ts waitForPhotoMessage`'s `${photosSel} img` + alt query still matches).

- [ ] **Step 1: Import BlobImage and swap the `<img>`**

In `PhotosAttachment.svelte`, add to the imports (after line 4):

```svelte
	import BlobImage from '$lib/components/BlobImage.svelte';
```

Replace the loop body (lines 35-42) so the raw `<img>` (line 37) becomes a `BlobImage`:

```svelte
	{#each photos as photo, i (i)}
		<button type="button" class="photo-cell" onclick={e => openLightbox(i, e)}>
			<BlobImage item={photo} alt={photo.name} />
			{#if i === 4 && photos.length > 5}
				<div class="photo-overlay">+{photos.length - 5}</div>
			{/if}
		</button>
	{/each}
```

(`mediaSrc`/`photoUrls` may now be unused — if `photoUrls` on line 31 is unused after this and Task 5, delete that line and the now-unused `mediaSrc` import to keep `pnpm check` clean. Verify usages before deleting.)

- [ ] **Step 2: Pierce the scoped img rules into the child component**

The img now lives inside `BlobImage`, so PhotosAttachment's scoped `.photo-cell img` selectors no longer match it. Add `:global()` to reach the child's `<img>` while staying anchored to the scoped `.photo-cell`.

Change line 173 from `.photo-cell img {` to:

```css
	.photo-cell :global(img) {
```

Change line 81 from `.photo-cell:only-child img {` to:

```css
	.photo-cell:only-child :global(img) {
```

- [ ] **Step 3: Keep the lone-image loading box from collapsing**

While a lone image is loading or errored there is no `<img>` to give the cell height, so the overlay/retry box would collapse. Add a min height to the only-child cell. Append inside the `.photo-cell:only-child img` block's neighbour — add a new rule after line 89:

```css
	.attachment-photos:has(.photo-cell:only-child) .photo-cell {
		min-width: 200px;
		min-height: 50px;
	}
```

- [ ] **Step 4: Type-check**

Run (from `ui/`): `nix develop --command pnpm check`
Expected: no new type errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/messages/attachments/PhotosAttachment.svelte
git commit -m "feat(ui): render photo grid cells via BlobImage

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Frontend — Lightbox uses BlobImage

**Files:**
- Modify: `ui/src/lib/components/messages/Lightbox.svelte:165-182` (stage + main image) and `:247-256` (`.lightbox-image` scoped rules)

**Interfaces:**
- Consumes: `BlobImage` (Task 3). The main image's Tailwind/zoom classes and the `transform-origin` inline style are forwarded via `imgClass`/`imgStyle`. The stage `<button>` becomes the positioning context (`relative`) for BlobImage's overlay. The filmstrip thumbnails (lines 224-228) are left as plain `<img>` (small, decorative; out of scope).
- Produces: no new symbols. Keeps `data-testid="lightbox-image"`? Note: BlobImage's img uses `data-testid="blob-image"`, not `lightbox-image`. The `lightbox.ts` page object queries `tid('lightbox-image')` (line 8) — preserve that id by passing it through.

- [ ] **Step 1: Decide testid + zoom-style handling**

BlobImage hardcodes `data-testid="blob-image"`, but `e2e-tests/helpers/components/lightbox.ts:8` expects `lightbox-image`. To avoid breaking that page object, keep the existing `lightbox-image` testid working: update `lightbox.ts` to also accept `blob-image` is brittle — instead, the simplest stable choice is to **point the lightbox page object at the alt-based image inside the lightbox**. Update `e2e-tests/helpers/components/lightbox.ts:8` from:

```ts
	image = this.agent.$(tid('lightbox-image'));
```

to:

```ts
	image = this.agent.$(`${tid('lightbox')} ${tid('blob-image')}`);
```

- [ ] **Step 2: Import BlobImage and replace the main image**

In `Lightbox.svelte`, add to the imports (after line 14):

```svelte
	import BlobImage from '$lib/components/BlobImage.svelte';
```

Replace the stage `<button>`/`<img>` (lines 165-182) so the stage is a positioning context and the image is a `BlobImage`:

```svelte
	<button
		type="button"
		class="relative flex min-h-0 flex-1 cursor-default items-center justify-center overflow-hidden border-none bg-transparent p-0"
		bind:this={stageEl}
		aria-label={m.closeLightbox()}
		onclick={onStageClick}
		ondblclick={onStageDoubleClick}
		onmousemove={onStageMouseMove}
	>
		<BlobImage
			item={photo}
			alt={photo.name}
			imgClass={`lightbox-image max-h-full max-w-full object-contain${zoomed ? ' zoomed' : ''}`}
			imgStyle={`transform-origin: ${originX}% ${originY}%`}
		/>
	</button>
```

- [ ] **Step 3: Make the lightbox image styles reach the child img**

The `.lightbox-image` / `.lightbox-image.zoomed` scoped rules (lines 247-256) target an element now rendered inside `BlobImage`. Convert them to `:global` (the classes are unique enough that global scope is safe). Replace lines 247-256 with:

```css
	:global(.lightbox-image) {
		transition: transform 0.15s ease;
	}
	:global(.lightbox-image.zoomed) {
		transform: scale(3);
		cursor: zoom-out;
	}
	:global(.lightbox-image:not(.zoomed)) {
		cursor: zoom-in;
	}
```

- [ ] **Step 4: Type-check**

Run (from `ui/`): `nix develop --command pnpm check`
Expected: no new type errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/messages/Lightbox.svelte e2e-tests/helpers/components/lightbox.ts
git commit -m "feat(ui): render lightbox image via BlobImage

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: E2E — retry hook + spec, and manual UI verification

**Files:**
- Modify: `ui/tests/setup-utils.ts` (add `forceBlobError` to `window.__test`)
- Modify: `e2e-tests/specs/media-attachments.spec.ts` (add a retry test) — confirm exact send/setup helpers from the existing spec before writing.

**Interfaces:**
- Consumes: existing `media-attachments.spec.ts` helpers for sending a photo and `agent.messages.waitForPhotoMessage(name)` (from `e2e-tests/helpers/components/messages.ts`). The new `window.__test.forceBlobError(alt)` dispatches the `test-blob-force-error` event BlobImage listens for (Task 3).
- Produces: `window.__test.forceBlobError(alt: string): void`.

- [ ] **Step 1: Add the test hook**

In `ui/tests/setup-utils.ts`, add a function near `simulateUpdate` (after line 22):

```ts
/** Force any BlobImage whose alt matches into its error/retry state. */
function forceBlobError(alt: string) {
	window.dispatchEvent(
		new CustomEvent('test-blob-force-error', { detail: alt }),
	);
}
```

and register it in the `testUtils` object (after `dropFiles,` on line 88):

```ts
	forceBlobError,
```

- [ ] **Step 2: Read the existing spec to mirror its setup**

Run: `sed -n '1,80p' e2e-tests/specs/media-attachments.spec.ts`
Expected: shows how a photo is sent (paste/drop via `window.__test.pasteFiles`/`dropFiles`) and how the receiving agent waits via `agent.messages.waitForPhotoMessage`. Use the same names below; adjust the test to match what you see.

- [ ] **Step 3: Write the retry test**

Add to `media-attachments.spec.ts` (adapt names to the spec's existing patterns and the sender/receiver agents it sets up):

```ts
it('shows a retry control and recovers after a failed image load', async () => {
	// (send a photo named e.g. "retry-me.png" using the same helper the
	// other tests in this file use, then:)
	await receiver.messages.waitForPhotoMessage('retry-me.png');

	// Force the rendered image into its error state.
	await receiver.execute((alt: string) => {
		window.__test.forceBlobError(alt);
	}, 'retry-me.png');

	const retry = receiver.$(tid('blob-image-retry'));
	await retry.waitForDisplayed();

	// Tapping retries; the blob is present locally, so it loads again.
	await retry.click();
	await receiver.messages.waitForPhotoMessage('retry-me.png');
});
```

(Import `tid` from `e2e-tests/helpers/selectors` if not already imported in the spec.)

- [ ] **Step 4: Build and run the spec**

Run: `nix develop --command just test e2e media-attachments`
Expected: the media-attachments suite passes, including the new retry test.

- [ ] **Step 5: Manual UI verification (REQUIRED by CLAUDE.md)**

- Use the `start-dev` skill to launch the app.
- Send a photo message between the two agents; confirm it renders (loaded state) in the grid and in the lightbox.
- In devtools console run `window.__test.forceBlobError('<the photo name>')`; confirm the retry placeholder appears, is centered, and is tappable; tap it and confirm the image reloads.
- Compare the photo grid + lightbox against `signal-reference` screenshots (`message-types/`) to confirm spacing/feel are unchanged.
- Toggle dark mode and confirm the retry placeholder and loading background read correctly.
- Kill all dev background processes when done.

- [ ] **Step 6: Commit**

```bash
git add ui/tests/setup-utils.ts e2e-tests/specs/media-attachments.spec.ts
git commit -m "test(e2e): cover BlobImage error and retry path

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- `load_blob` `Option<Duration>` (None = one-shot) → Task 1. ✓
- 250ms poll, 10s deadline, passive (no enqueue) → Task 1 (poll) + Task 2 (10s). ✓
- 404 `Cache-Control: no-store`, 200 stays cacheable → Task 2. ✓
- Shared `BlobImage` with loading/loaded/error, real `<img>` + `alt`, labeled retry button → Task 3. ✓
- `Date.now()` buster, query-free first load → Task 3. ✓
- No auto-retry loop (manual tap only) → Task 3 (error state terminal until `retry()`). ✓
- Styling boundary (`:global` to pierce child img / consumer owns layout) → Tasks 4 (`:global(img)`) and 5 (`:global(.lightbox-image)`). ✓
- Consumers PhotosAttachment + Lightbox → Tasks 4, 5. ✓
- Rust tests (absent/None, deadline, lands-mid-wait) → Task 1. ✓
- E2E error→retry hook → Task 6. ✓
- Manual UI verification → Task 6 Step 5. ✓
- review-checks: BlobImage is a component, not a route → no visit-all-pages change (noted in spec). ✓

**Placeholder scan:** Task 6's spec body is intentionally adaptive (Step 2 reads the real spec first) because the send helper names depend on the existing file; every other step has concrete code. No TBD/TODO left in code steps.

**Type consistency:** `load_blob(&str, Option<std::time::Duration>)` used identically in Tasks 1 and 2. `BlobImage` props `{ item, alt, imgClass, imgStyle }` and testids `blob-image` / `blob-image-loading` / `blob-image-retry` are consistent across Tasks 3–6. `test-blob-force-error` event + `forceBlobError(alt)` consistent between Tasks 3 and 6. `lightbox.ts` image selector updated in Task 5 to match the new `blob-image` testid.
