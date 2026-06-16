<script lang="ts">
	import type { Media } from 'dash-chat-stores';
	import {
		byteLengthOf,
		bytesToBlobUrl,
		formatFileSize,
	} from '$lib/types/media';
	import { getTimelineImageDimensions, gridConfig } from './photo-grid';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import { saveAttachment } from '$lib/utils/save-file';
	import Lightbox from './Lightbox.svelte';

	interface Props {
		media: Media;
		/** Display name of the message author, shown in the lightbox header. */
		senderName?: string;
		timestamp?: number;
	}

	let { media, senderName = '', timestamp = 0 }: Props = $props();

	// `null` while closed; the triggering element is remembered so focus can be
	// restored to it on close.
	let lightboxIndex = $state<number | null>(null);
	let lightboxTrigger: HTMLElement | undefined;

	function openLightbox(index: number, event: MouseEvent) {
		if (media.kind !== 'photos') return;
		lightboxTrigger = event.currentTarget as HTMLElement;
		lightboxIndex = index;
	}

	function closeLightbox() {
		lightboxIndex = null;
		lightboxTrigger?.focus();
		lightboxTrigger = undefined;
	}

	// Build object URLs once per Media instance. Minting and revoking live
	// in the same pre-effect (not a $derived) so the URLs can never leak if
	// a derived were to re-evaluate independently of its consumer.
	// The keyed {#each (photoUrls[i])} below relies on the pre-effect
	// repopulating photoUrls before the DOM updates.
	let photoUrls = $state<string[]>([]);

	$effect.pre(() => {
		const urls =
			media.kind === 'photos'
				? media.photos.map(p => bytesToBlobUrl(p.data, p.mime_type))
				: [];
		photoUrls = urls;
		return () => urls.forEach(u => URL.revokeObjectURL(u));
	});

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
			<span class="attachment-file-size"
				>{formatFileSize(byteLengthOf(file.data))}</span
			>
		</div>
	</button>
{/if}

{#if lightboxIndex !== null && media.kind === 'photos'}
	<Lightbox
		photos={media.photos}
		index={lightboxIndex}
		{senderName}
		{timestamp}
		onClose={closeLightbox}
	/>
{/if}

<style>
	.attachment-photos {
		position: relative;
		max-width: 100%;
		background: white;
	}

	/* Plate behind transparent images. */
	:global(.dark) .attachment-photos {
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
		grid-template-rows: 1fr;
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
		height: 100%;
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
