<script lang="ts">
	import type { Media } from 'dash-chat-stores';
	import { formatFileSize, mediaSize, mediaSrc } from '$lib/types/media';
	import { getTimelineImageDimensions, gridConfig } from './photo-grid';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import { saveAttachment } from '$lib/utils/save-file';
	import { lightbox } from '$lib/stores/lightbox.svelte';

	interface Props {
		media: Media;
		/** True when the bubble renders content (e.g. a sender header) above
		 * the media, squaring the media's top corners. */
		withContentAbove?: boolean;
		/** True when the bubble renders content (caption or timestamp row)
		 * below the media, squaring the media's bottom corners. */
		withContentBelow?: boolean;
		/** Display name of the message author, shown in the lightbox header. */
		senderName?: string;
		timestamp?: number;
	}

	let {
		media,
		withContentAbove = false,
		withContentBelow = false,
		senderName = '',
		timestamp = 0,
	}: Props = $props();

	function openLightbox(index: number, event: MouseEvent) {
		if (media.kind !== 'photos') return;
		lightbox.open(
			{ photos: media.photos, index, senderName, timestamp },
			event.currentTarget as HTMLElement,
		);
	}

	// Stable `irohblob://` URLs served from the node's local blob store; the
	// webview caches them (content-addressed hashes are immutable).
	const photoUrls = $derived(
		media.kind === 'photos' ? media.photos.map(mediaSrc) : [],
	);

	// Lone images render at Signal's timeline size for their natural aspect
	// ratio; until decode (local bytes, effectively instant) use the minimum.
	let singleDims = $state({ width: 200, height: 50 });

	function onSingleLoad(event: Event) {
		const img = event.currentTarget as HTMLImageElement;
		singleDims = getTimelineImageDimensions(
			img.naturalWidth,
			img.naturalHeight,
		);
	}
</script>

{#if media.kind === 'photos'}
	{@const photos = media.photos}
	{#if photos.length === 1}
		<div
			class="attachment-photos single"
			class:with-content-above={withContentAbove}
			class:with-content-below={withContentBelow}
			style="width: {singleDims.width}px; height: {singleDims.height}px"
			data-testid="message-attachment-photos"
		>
			<button
				type="button"
				class="photo-cell"
				onclick={e => openLightbox(0, e)}
			>
				<img src={photoUrls[0]} alt={photos[0].name} onload={onSingleLoad} />
			</button>
		</div>
	{:else}
		{@const cfg = gridConfig(photos.length)}
		{@const overflow = photos.length - cfg.visibleCells}
		<div
			class="attachment-photos multi cells-{cfg.visibleCells}"
			class:with-content-above={withContentAbove}
			class:with-content-below={withContentBelow}
			style="aspect-ratio: {cfg.aspectRatio}"
			data-testid="message-attachment-photos"
		>
			{#each photos.slice(0, cfg.visibleCells) as photo, i (photoUrls[i])}
				<button
					type="button"
					class="photo-cell"
					onclick={e => openLightbox(i, e)}
				>
					<img src={photoUrls[i]} alt={photo.name} loading="lazy" />
					{#if i === cfg.visibleCells - 1 && overflow > 0}
						<div class="photo-overlay">+{overflow}</div>
					{/if}
				</button>
			{/each}
		</div>
	{/if}
{:else}
	{@const file = media.file}
	<button
		type="button"
		class="attachment-file"
		data-testid="message-attachment-file"
		onclick={() => saveAttachment(file)}
	>
		<div class="attachment-file-icon">
			<ExtensionSheet name={file.name} />
		</div>
		<div class="attachment-file-info">
			<span class="attachment-file-name">{file.name}</span>
			<span class="attachment-file-size">{formatFileSize(mediaSize(file))}</span
			>
		</div>
	</button>
{/if}

<style>
	.attachment-photos {
		position: relative;
		/* Pull to bubble edge; the bubble's inner padding wraps text-only
		 * messages but media should be edge-to-edge. The corner radius is
		 * inherited so the media matches the bubble's border. */
		margin: calc(-1 * var(--bubble-padding, 0.5rem));
		max-width: calc(100% + 2 * var(--bubble-padding, 0.5rem));
		border-radius: inherit;
		overflow: hidden;
	}

	/* Hairline border so near-white images don't bleed into the bubble. */
	.attachment-photos::after {
		content: '';
		position: absolute;
		inset: 0;
		border-radius: inherit;
		box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
		pointer-events: none;
	}

	.attachment-photos.with-content-above {
		margin-top: 0;
		border-start-start-radius: 0;
		border-start-end-radius: 0;
	}

	.attachment-photos.with-content-below {
		margin-bottom: 4px;
		border-end-start-radius: 0;
		border-end-end-radius: 0;
	}

	/* Plate behind transparent images. */
	.single {
		background: white;
	}
	:global(.dark) .single {
		background: black;
	}

	.single .photo-cell {
		width: 100%;
		height: 100%;
		background: transparent;
	}

	.multi {
		display: grid;
		gap: 1px;
		width: 300px;
	}

	.cells-2 {
		grid-template-columns: 1fr 1fr;
	}

	.cells-3 {
		grid-template-columns: 2fr 1fr;
		grid-template-rows: 1fr 1fr;
		grid-template-areas:
			'a b'
			'a c';
	}
	.cells-3 .photo-cell:nth-child(1) {
		grid-area: a;
	}
	.cells-3 .photo-cell:nth-child(2) {
		grid-area: b;
	}
	.cells-3 .photo-cell:nth-child(3) {
		grid-area: c;
	}

	.cells-4 {
		grid-template-columns: 1fr 1fr;
		grid-template-rows: 1fr 1fr;
	}

	.cells-5 {
		grid-template-columns: repeat(6, 1fr);
		grid-template-rows: 3fr 2fr;
		grid-template-areas:
			'a a a b b b'
			'c c d d e e';
	}
	.cells-5 .photo-cell:nth-child(1) {
		grid-area: a;
	}
	.cells-5 .photo-cell:nth-child(2) {
		grid-area: b;
	}
	.cells-5 .photo-cell:nth-child(3) {
		grid-area: c;
	}
	.cells-5 .photo-cell:nth-child(4) {
		grid-area: d;
	}
	.cells-5 .photo-cell:nth-child(5) {
		grid-area: e;
	}

	.photo-cell {
		position: relative;
		overflow: hidden;
		background: rgba(128, 128, 128, 0.08);
		border: none;
		padding: 0;
		display: block;
		cursor: pointer;
	}

	.photo-cell img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
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

	.attachment-file-icon {
		width: 36px;
		height: 40px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-inline-end: 0.75rem;
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
</style>
