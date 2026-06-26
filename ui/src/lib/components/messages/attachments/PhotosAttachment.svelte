<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
	import { pushState } from '$app/navigation';
	import { page } from '$app/state';
	import Lightbox from '../Lightbox.svelte';
	import PhotoAttachmentGallery from './PhotoAttachmentGallery.svelte';

	interface Props {
		photos: PhotoAttachment[];
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
		// Open behind a pushed history entry so the Android system back button
		// (and the browser back button) closes the lightbox instead of leaving
		// the chat. Closing pops the entry; the effect below reacts either way.
		pushState('', { lightbox: true });
	}

	function closeLightbox() {
		lightboxIndex = null;
		lightboxTrigger?.focus();
		lightboxTrigger = undefined;
	}

	$effect(() => {
		if (lightboxIndex !== null && !page.state.lightbox) closeLightbox();
	});
</script>

<div style="max-width: 280px">
	<PhotoAttachmentGallery {photos} onPhotoClick={openLightbox} />
</div>

{#if lightboxIndex !== null}
	<Lightbox
		{photos}
		index={lightboxIndex}
		{senderName}
		{timestamp}
		onClose={() => history.back()}
	/>
{/if}
