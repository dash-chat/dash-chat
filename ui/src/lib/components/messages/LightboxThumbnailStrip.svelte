<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
	import BlobImage from '$lib/components/BlobImage.svelte';
	import { blobStatus, retryBlob } from '$lib/stores/blob-load-store.svelte';

	interface Props {
		photos: PhotoAttachment[];
		/** Index of the active photo; its thumbnail is enlarged and centred.
		    Bound, so clicking a thumbnail updates it directly. */
		index?: number;
		/** Hidden (e.g. while the photo is zoomed). */
		faded?: boolean;
	}

	let { photos, index = $bindable(0), faded = false }: Props = $props();

	let stripEl: HTMLElement | undefined = $state();

	// A failed thumbnail shows a reload icon; clicking it retries the download and
	// switches to that photo. Load state is shared per blob hash, so the retry
	// re-fetches the main-stage image of the same photo alongside the thumbnail.
	// Guard on the error status so retrying a healthy thumb can't needlessly bust
	// its cache and re-fetch.
	function onThumbClick(i: number) {
		if (blobStatus(photos[i].hash) === 'error') retryBlob(photos[i].hash);
		index = i;
	}

	// Keep the active thumbnail centred as the selection changes.
	$effect(() => {
		stripEl?.children[index]?.scrollIntoView({
			behavior: 'smooth',
			inline: 'center',
			block: 'nearest',
		});
	});
</script>

<div
	bind:this={stripEl}
	class="lightbox-filmstrip flex shrink-0 touch-pan-x items-center justify-center-safe gap-2 overflow-x-auto overscroll-x-contain px-3 pt-2.5"
	class:faded
	data-testid="lightbox-filmstrip"
>
	{#each photos as p, i (i)}
		<button
			type="button"
			class="lightbox-thumb relative h-11 w-11 shrink-0 overflow-hidden p-0"
			class:selected={i === index}
			data-testid="lightbox-thumb-{i}"
			aria-label={p.name}
			onclick={() => onThumbClick(i)}
		>
			<BlobImage
				item={p}
				alt={p.name}
				imgClass="block h-full w-full object-cover"
				lazy
			/>
		</button>
	{/each}
</div>

<style>
	.lightbox-filmstrip {
		padding-bottom: 0.625rem;
		transition: opacity 0.15s ease;
	}

	.lightbox-thumb {
		border: none;
		border-radius: 6px;
		cursor: pointer;
		background: transparent;
		opacity: 0.5;
		transform-origin: center;
		transition:
			opacity 0.15s ease,
			transform 0.15s ease;
	}
	.lightbox-thumb:hover {
		opacity: 0.8;
	}
	.lightbox-thumb.selected {
		opacity: 1;
		transform: scale(1.25);
	}

	.faded {
		opacity: 0;
		pointer-events: none;
	}
</style>
