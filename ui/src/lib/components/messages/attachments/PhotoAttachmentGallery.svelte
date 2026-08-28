<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
	import BlobImage from '$lib/components/BlobImage.svelte';
	import { timelineImageBox } from '$lib/utils/media';

	interface Props {
		photos: PhotoAttachment[];
		/** Called when a loaded photo cell is clicked, e.g. to open a lightbox. */
		onPhotoClick: (index: number, event: MouseEvent) => void;
	}

	let { photos, onPhotoClick }: Props = $props();

	// Cell component handles, so a click on a cell whose image failed to load
	// retries the download instead of opening the lightbox.
	const blobImages = $state<Record<number, { retryIfErrored: () => boolean }>>(
		{},
	);

	// The 5th cell of a 6+ gallery is the "+N" overflow scrim — it stands for all
	// the hidden photos, so it always opens the lightbox rather than retrying its
	// own thumbnail.
	const isOverflowCell = (index: number) => index === 4 && photos.length > 5;

	const loneBox = $derived(
		photos.length === 1 ? timelineImageBox(photos[0]) : null,
	);

	function onCellClick(index: number, event: MouseEvent) {
		if (!isOverflowCell(index) && blobImages[index]?.retryIfErrored()) return;
		onPhotoClick(index, event);
	}
</script>

<div class="attachment-photos" data-testid="message-attachment-photos">
	{#each photos as photo, i (i)}
		<button
			type="button"
			class="photo-cell"
			style={loneBox !== null
				? `width: ${loneBox.width}px; height: ${loneBox.height}px;`
				: ''}
			aria-label={photo.name}
			onclick={e => onCellClick(i, e)}
		>
			<BlobImage bind:this={blobImages[i]} item={photo} alt={photo.name} lazy />
			{#if isOverflowCell(i)}
				<div class="photo-overlay">+{photos.length - 5}</div>
			{/if}
		</button>
	{/each}
</div>

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

	/*
		One markup path for any photo count; the whole layout is picked purely
		in CSS from the number of cells (no JS). A `:has()` quantity query
		matches each case: a lone image gets a fixed box from its sender-measured
		dimensions, while 2+ images form Signal's collages in a 300px-wide grid. The 5+ rule also
		hides the 6th-and-later cells and reveals the +N scrim on the 5th.
	*/

	/* 1 → lone image: the cell carries its fixed box (from timelineImageBox)
	 * inline, so it is right before the blob loads and never changes when it
	 * arrives. The image is absolutely positioned to cover the box without
	 * contributing its natural size to the bubble width. */
	.attachment-photos:has(.photo-cell:only-child) {
		width: fit-content;
	}
	.photo-cell:only-child :global(img) {
		position: absolute;
		inset: 0;
	}

	/* 2+ → a 300px-wide collage grid */
	.attachment-photos:has(.photo-cell:nth-child(2)) {
		display: grid;
		gap: 2px;
		width: 300px;
	}

	/* 2 → side by side */
	.attachment-photos:has(.photo-cell:nth-child(2):last-child) {
		aspect-ratio: 2 / 1;
		grid-template-columns: 1fr 1fr;
		grid-template-rows: 1fr;
	}

	/* 3 → one tall on the start, two stacked on the end */
	.attachment-photos:has(.photo-cell:nth-child(3):last-child) {
		aspect-ratio: 3 / 2;
		grid-template-columns: 2fr 1fr;
		grid-template-rows: 1fr 1fr;
		grid-template-areas:
			'a b'
			'a c';
	}
	.attachment-photos:has(.photo-cell:nth-child(3):last-child)
		.photo-cell:nth-child(1) {
		grid-area: a;
	}
	.attachment-photos:has(.photo-cell:nth-child(3):last-child)
		.photo-cell:nth-child(2) {
		grid-area: b;
	}
	.attachment-photos:has(.photo-cell:nth-child(3):last-child)
		.photo-cell:nth-child(3) {
		grid-area: c;
	}

	/* 4 → 2×2 */
	.attachment-photos:has(.photo-cell:nth-child(4):last-child) {
		aspect-ratio: 1 / 1;
		grid-template-columns: 1fr 1fr;
		grid-template-rows: 1fr 1fr;
	}

	/* 5+ → two over three, extras hidden behind a +N scrim on the 5th */
	.attachment-photos:has(.photo-cell:nth-child(5)) {
		aspect-ratio: 6 / 5;
		grid-template-columns: repeat(6, 1fr);
		grid-template-rows: 3fr 2fr;
		grid-template-areas:
			'a a a b b b'
			'c c d d e e';
	}
	.attachment-photos:has(.photo-cell:nth-child(5)) .photo-cell:nth-child(1) {
		grid-area: a;
	}
	.attachment-photos:has(.photo-cell:nth-child(5)) .photo-cell:nth-child(2) {
		grid-area: b;
	}
	.attachment-photos:has(.photo-cell:nth-child(5)) .photo-cell:nth-child(3) {
		grid-area: c;
	}
	.attachment-photos:has(.photo-cell:nth-child(5)) .photo-cell:nth-child(4) {
		grid-area: d;
	}
	.attachment-photos:has(.photo-cell:nth-child(5)) .photo-cell:nth-child(5) {
		grid-area: e;
	}
	.attachment-photos .photo-cell:nth-child(n + 6) {
		display: none;
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

	.photo-cell :global(img) {
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
</style>
