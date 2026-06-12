<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { FileAttachment, Media, Photo } from 'dash-chat-stores';
	import { mdiDownload, mdiFile } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		asUint8Array,
		byteLengthOf,
		bytesToBlobUrl,
		formatFileSize,
	} from '$lib/types/media';
	import { isTauriEnv } from '$lib/utils/environment';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';

	let { media }: { media: Media } = $props();

	// Build object URLs once per Media instance and revoke on teardown.
	const photoUrls = $derived.by(() =>
		media.kind === 'photos'
			? media.photos.map(p => bytesToBlobUrl(p.data, p.mime_type))
			: [],
	);

	$effect(() => {
		// Capture for cleanup so we revoke exactly what we created.
		const urls = photoUrls;
		return () => urls.forEach(u => URL.revokeObjectURL(u));
	});

	function totalSize(photos: Photo[]): number {
		return photos.reduce((n, p) => n + byteLengthOf(p.data), 0);
	}
	void totalSize; // reserved for future use (e.g. size badge)

	async function saveFile(file: FileAttachment): Promise<void> {
		try {
			if (isTauriEnv()) {
				const [{ save }, { writeFile }, { downloadDir, join }] =
					await Promise.all([
						import('@tauri-apps/plugin-dialog'),
						import('@tauri-apps/plugin-fs'),
						import('@tauri-apps/api/path'),
					]);
				let defaultPath = file.name;
				try {
					defaultPath = await join(await downloadDir(), file.name);
				} catch {
					// downloadDir may not exist on some platforms; fall back to bare name
				}
				const path = await save({ title: m.saveFile(), defaultPath });
				if (!path) return;
				await writeFile(path, asUint8Array(file.data));
				showToast(m.fileSaved());
			} else {
				const url = bytesToBlobUrl(file.data, file.mime_type);
				const a = document.createElement('a');
				a.href = url;
				a.download = file.name;
				a.click();
				URL.revokeObjectURL(url);
			}
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	// Number of photos to actually render in the grid (others become an
	// overlay count on the last visible photo).
	const MAX_PHOTO_CELLS = 4;
</script>

{#if media.kind === 'photos'}
	{@const photos = media.photos}
	{@const n = photos.length}
	{@const cells = Math.min(n, MAX_PHOTO_CELLS)}
	{@const overflow = n - cells}
	<div
		class="attachment-photos"
		class:photos-1={cells === 1}
		class:photos-2={cells === 2}
		class:photos-3={cells === 3}
		class:photos-4={cells === 4}
		data-testid="message-attachment-photos"
	>
		{#each photos.slice(0, cells) as photo, i (photoUrls[i])}
			<div
				class="photo-cell"
				class:photo-cell-overlay={i === cells - 1 && overflow > 0}
			>
				<img src={photoUrls[i]} alt={photo.name} loading="lazy" />
				{#if i === cells - 1 && overflow > 0}
					<div class="photo-overlay">+{overflow}</div>
				{/if}
			</div>
		{/each}
	</div>
{:else}
	{@const file = media.file}
	<button
		type="button"
		class="attachment-file"
		data-testid="message-attachment-file"
		onclick={() => saveFile(file)}
	>
		<wa-icon src={wrapPathInSvg(mdiFile)} class="attachment-file-icon"
		></wa-icon>
		<div class="attachment-file-info">
			<span class="attachment-file-name">{file.name}</span>
			<span class="attachment-file-size"
				>{formatFileSize(byteLengthOf(file.data))}</span
			>
		</div>
		<wa-icon src={wrapPathInSvg(mdiDownload)} class="attachment-file-download"
		></wa-icon>
	</button>
{/if}

<style>
	.attachment-photos {
		display: grid;
		gap: 2px;
		/* Pull to bubble edge; the bubble's inner padding wraps text-only
		 * messages but media should be edge-to-edge. The corner radius is
		 * inherited so the grid matches the bubble's border. */
		margin: calc(-1 * var(--bubble-padding, 0.5rem))
			calc(-1 * var(--bubble-padding, 0.5rem)) 4px;
		border-radius: inherit;
		overflow: hidden;
		/* Give multi-cell grids a useful width even when the bubble's text
		 * content would otherwise shrink it. */
		min-width: 240px;
	}

	.photos-1 {
		grid-template-columns: 1fr;
		min-width: 0;
	}
	.photos-2 {
		grid-template-columns: 1fr 1fr;
	}
	.photos-3 {
		grid-template-columns: 1fr 1fr;
		grid-template-areas:
			'a a'
			'b c';
	}
	.photos-3 .photo-cell:nth-child(1) {
		grid-area: a;
	}
	.photos-3 .photo-cell:nth-child(2) {
		grid-area: b;
	}
	.photos-3 .photo-cell:nth-child(3) {
		grid-area: c;
	}
	.photos-4 {
		grid-template-columns: 1fr 1fr;
		grid-template-rows: 1fr 1fr;
	}

	.photo-cell {
		position: relative;
		overflow: hidden;
		aspect-ratio: 1;
		background: rgba(0, 0, 0, 0.05);
	}
	/* Single-photo bubbles use the image's natural aspect ratio (capped by
	 * max-height) rather than forcing a square crop. The image drives the
	 * height; width fills the bubble so a 200×150 photo doesn't collapse
	 * to thumbnail size. */
	.photos-1 .photo-cell {
		aspect-ratio: auto;
		background: transparent;
	}
	.photos-3 .photo-cell:nth-child(1) {
		aspect-ratio: 2 / 1;
	}

	.photo-cell img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.photos-1 .photo-cell img {
		height: auto;
		max-width: 100%;
		max-height: 320px;
		object-fit: cover;
	}

	.photo-overlay {
		position: absolute;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 24px;
		font-weight: 600;
	}

	.attachment-file {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 10px;
		margin: -4px 0 6px;
		border: none;
		border-radius: 10px;
		background: rgba(255, 255, 255, 0.18);
		cursor: pointer;
		text-align: start;
		color: inherit;
		transition: background-color 0.1s ease;
	}
	.attachment-file:hover {
		background: rgba(255, 255, 255, 0.28);
	}

	/* Others-message bubble has a light background; use a darker overlay */
	:global(.others-message) .attachment-file {
		background: rgba(0, 0, 0, 0.05);
	}
	:global(.others-message) .attachment-file:hover {
		background: rgba(0, 0, 0, 0.1);
	}

	.attachment-file :global(.attachment-file-icon) {
		width: 28px;
		height: 28px;
		opacity: 0.85;
		flex-shrink: 0;
	}

	.attachment-file-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.attachment-file-name {
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.attachment-file-size {
		font-size: 12px;
		opacity: 0.7;
	}

	.attachment-file :global(.attachment-file-download) {
		width: 20px;
		height: 20px;
		opacity: 0.7;
		flex-shrink: 0;
	}
</style>
