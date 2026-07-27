<script lang="ts">
	import { type DraftMedia } from '$lib/utils/media';
	import { objectUrl } from '$lib/actions/object-url';
	import { renderAboveKeyboard } from '$lib/utils/virtual-keyboard/render-above-keyboard';
	import ImageCarousel from '$lib/components/ImageCarousel.svelte';

	interface Props {
		/** The staged draft. Only the `photos` variant is rendered. */
		media: DraftMedia | undefined;
		/** Index of the currently shown photo. */
		index?: number;
	}

	let { media = $bindable(), index = $bindable(0) }: Props = $props();

	const photos = $derived(media?.kind === 'photos' ? media.items : []);

	// Keep the selected index in range as photos are added or removed.
	$effect(() => {
		if (index > photos.length - 1) index = Math.max(0, photos.length - 1);
	});

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
	}

	function onKeydown(event: KeyboardEvent) {
		// Don't hijack cursor movement while typing the caption.
		if (event.target instanceof HTMLTextAreaElement) return;
		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			select(index - 1);
		} else if (event.key === 'ArrowRight') {
			event.preventDefault();
			select(index + 1);
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<ImageCarousel
	bind:index
	items={photos}
	key={photo => photo}
	class="min-h-0 flex-1"
>
	{#snippet slide(photo)}
		<img
			class="rounded-2xl object-contain"
			style="max-height: 70vh; max-width: 70vw;"
			use:objectUrl={photo}
			use:renderAboveKeyboard
			alt={photo.name}
		/>
	{/snippet}
</ImageCarousel>
