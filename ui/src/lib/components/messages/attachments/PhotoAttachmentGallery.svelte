<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
	import BlobImage from '$lib/components/BlobImage.svelte';

	interface Props {
		photos: PhotoAttachment[];
		/** Called when a loaded photo cell is clicked, e.g. to open a lightbox. */
		onPhotoClick: (index: number, event: MouseEvent) => void;
	}

	let { photos, onPhotoClick }: Props = $props();

	// Per-cell load state and component handles, so a click on a cell whose
	// image failed to load retries the download instead of opening the lightbox.
	let statuses = $state<Record<number, 'loading' | 'loaded' | 'error'>>({});
	const blobImages: Record<number, { retry: () => void }> = {};

	// The 5th cell of a 6+ gallery is the "+N" overflow scrim — it stands for all
	// the hidden photos, so it always opens the lightbox rather than retrying its
	// own thumbnail.
	const isOverflowCell = (index: number) => index === 4 && photos.length > 5;

	function onCellClick(index: number, event: MouseEvent) {
		if (statuses[index] === 'error' && !isOverflowCell(index)) {
			blobImages[index]?.retry();
		} else {
			onPhotoClick(index, event);
		}
	}
</script>

<div class="attachment-photos" data-testid="message-attachment-photos">
	{#each photos as photo, i (i)}
		<button
			type="button"
			class="photo-cell"
			aria-label={photo.name}
			onclick={e => onCellClick(i, e)}
		>
			<BlobImage
				bind:this={blobImages[i]}
				item={photo}
				alt={photo.name}
				lazy
				onStatus={s => (statuses[i] = s)}
			/>
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
		matches each case: a lone image keeps its own aspect ratio, while 2+
		images form Signal's collages in a 300px-wide grid. The 5+ rule also
		hides the 6th-and-later cells and reveals the +N scrim on the 5th.
	*/

	/* 1 → lone image: container shrinks to it; the image keeps its own aspect
	 * ratio within Signal's timeline bounds (width 200–300px, height 50–450px),
	 * the browser scaling the unconstrained dimension to preserve the ratio. */
	.attachment-photos:has(.photo-cell:only-child) {
		width: fit-content;
	}
	.photo-cell:only-child :global(img) {
		width: auto;
		height: auto;
		min-width: 200px;
		max-width: 300px;
		min-height: 50px;
		max-height: 450px;
		object-fit: contain;
	}

	/* Floor the cell's size before the img exists (loading/error), so the overlay/retry box doesn't collapse. */
	.attachment-photos:has(.photo-cell:only-child) .photo-cell {
		min-width: 200px;
		min-height: 150px;
	}

	/* On error there's no <img> to size the container, so fit-content floors it
	 * to 200px. When a wider caption stretches the bubble, that leaves a white
	 * strip beside the grey placeholder — let it fill the bubble width instead.
	 * The cell is a <button>, which shrinks to content, so it needs width:100%
	 * to fill the stretched container too. */
	.attachment-photos:has(.photo-cell:only-child):not(:has(img)) {
		width: auto;
	}
	.attachment-photos:has(.photo-cell:only-child):not(:has(img)) .photo-cell {
		width: 100%;
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
