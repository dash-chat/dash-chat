<script lang="ts">
	import type { Photo } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/types/media';
	import { objectUrl } from '$lib/actions/object-url';
	import Lightbox from '../Lightbox.svelte';

	interface Props {
		photos: Photo[];
		/** Display name of the message author, shown in the lightbox header. */
		senderName?: string;
		timestamp?: number;
	}

	let { photos, senderName = '', timestamp = 0 }: Props = $props();

	// `null` while closed; the triggering element is remembered so focus can be
	// restored to it on close.
	let lightboxIndex = $state<number | null>(null);
	let lightboxTrigger: HTMLElement | undefined;

	function openLightbox(index: number, event: MouseEvent) {
		lightboxTrigger = event.currentTarget as HTMLElement;
		lightboxIndex = index;
	}

	function closeLightbox() {
		lightboxIndex = null;
		lightboxTrigger?.focus();
		lightboxTrigger = undefined;
	}

	const photoUrls = $derived(photos.map(mediaSrc));
</script>

<div class="attachment-photos" data-testid="message-attachment-photos">
	{#each photos as photo, i (i)}
		<button type="button" class="photo-cell" onclick={e => openLightbox(i, e)}>
			<img
				use:objectUrl={{ data: photo.data, mimeType: photo.mime_type }}
				alt={photo.name}
				loading="lazy"
			/>
			{#if i === 4 && photos.length > 5}
				<div class="photo-overlay">+{photos.length - 5}</div>
			{/if}
		</button>
	{/each}
</div>

{#if lightboxIndex !== null}
	<Lightbox
		{photos}
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
	.photo-cell:only-child img {
		width: auto;
		height: auto;
		min-width: 200px;
		max-width: 300px;
		min-height: 50px;
		max-height: 450px;
		object-fit: contain;
	}

	/* 2+ → a 300px-wide collage grid */
	.attachment-photos:has(.photo-cell:nth-child(2)) {
		display: grid;
		gap: 1px;
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
</style>
