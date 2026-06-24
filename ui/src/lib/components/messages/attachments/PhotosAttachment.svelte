<script lang="ts">
	import type { PhotoAttachment } from 'dash-chat-stores';
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
	}

	function closeLightbox() {
		lightboxIndex = null;
		lightboxTrigger?.focus();
		lightboxTrigger = undefined;
	}
</script>

<PhotoAttachmentGallery {photos} onPhotoClick={openLightbox} />

{#if lightboxIndex !== null}
	<Lightbox
		{photos}
		index={lightboxIndex}
		{senderName}
		{timestamp}
		onClose={closeLightbox}
	/>
{/if}
