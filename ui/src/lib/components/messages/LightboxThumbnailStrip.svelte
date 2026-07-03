<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
	import BlobImage from '$lib/components/BlobImage.svelte';

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
	const thumbs: Record<number, { retryIfErrored: () => boolean }> = {};

	// A failed thumbnail shows a reload icon; clicking it retries the download
	// (which re-fetches the main-stage image of the same blob too) and switches to
	// that photo. A healthy thumb just switches — `retryIfErrored` no-ops.
	function onThumbClick(i: number) {
		thumbs[i]?.retryIfErrored();
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
				bind:this={thumbs[i]}
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
