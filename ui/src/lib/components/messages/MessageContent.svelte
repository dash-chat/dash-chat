<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Message } from 'dash-chat-stores';
	import { senderColor } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';
	import PhotosAttachment from './attachments/PhotosAttachment.svelte';
	import FileAttachment from './attachments/FileAttachment.svelte';
	import MessageText from './MessageText.svelte';

	let {
		message,
		searchQuery,
		metadata,
		editedIndicator,
		senderName = '',
		showSenderName = false,
	}: {
		message: Message;
		searchQuery: string;
		metadata?: Snippet;
		editedIndicator?: Snippet;
		/** Author display name; shown as the bubble header (in groups) and used
		 * as the lightbox title. */
		senderName?: string;
		showSenderName?: boolean;
	} = $props();

	const media = $derived(message.content.media);
	const hasText = $derived(!!message.content.message);
	const isPhotoOnly = $derived(media?.kind === 'photos' && !hasText);
	const isFileOnly = $derived(media?.kind === 'file' && !hasText);

	let metadataWidth = $state(0);
</script>

{#if showSenderName}
	<div
		class="sender-name"
		style="color: {senderColor(message.author)}"
		data-testid="group-message-sender-name"
	>
		{senderName}
	</div>
{/if}
{#if media?.kind === 'photos'}
	<div class="media photos">
		<PhotosAttachment
			photos={media.photos}
			{senderName}
			timestamp={message.timestamp}
		/>
		{#if isPhotoOnly && (metadata || editedIndicator)}
			<div
				class="photo-meta pointer-events-none absolute inset-x-0 bottom-0 flex items-center justify-end gap-1 px-2 pt-4 pb-1"
			>
				{#if editedIndicator}
					{@render editedIndicator()}
				{/if}
				{#if metadata}
					{@render metadata()}
				{/if}
			</div>
		{/if}
	</div>
{:else if media?.kind === 'file'}
	<div class="media file">
		<FileAttachment
			file={media.file}
			metadata={isFileOnly ? metadata : undefined}
		/>
	</div>
{/if}
{#if hasText || (metadata && !isPhotoOnly && !isFileOnly) || (editedIndicator && !isPhotoOnly && !isFileOnly)}
	<div class="caption relative px-1">
		{#if metadata || editedIndicator}
			<div
				class="absolute bottom-0 end-0 flex items-center gap-1 whitespace-nowrap select-none"
				bind:clientWidth={metadataWidth}
			>
				{#if editedIndicator}
					{@render editedIndicator()}
				{/if}
				{#if metadata}
					{@render metadata()}
				{/if}
			</div>
		{/if}
		<div class="max-w-full" use:shrinkToWidestLine>
			<MessageText text={message.content.message} {searchQuery} />
			<!-- Reserves the metadata's space in the bottom-end corner, since
			     wrapped text cannot be made to avoid an absolute box via CSS. -->
			{#if metadata || editedIndicator}
				<span class="ms-2.5 inline-block" style="width: {metadataWidth}px"
				></span>
			{/if}
		</div>
	</div>
{/if}

<style>
	.sender-name {
		margin-inline: 0.25rem;
		margin-bottom: 0.125rem;
		font-size: 0.875rem;
		font-weight: 600;
		text-align: start;
	}

	/* Photos bleed past the bubble's 0.5rem padding to sit edge-to-edge; the
	 * Card's overflow clip rounds the corners to match the bubble. */
	.media {
		position: relative;
	}
	.media.photos {
		margin: -0.5rem;
		max-width: calc(100% + 1rem);
	}
	/* Hairline border so near-white images don't bleed into the bubble. */
	.media.photos::after {
		content: '';
		position: absolute;
		inset: 0;
		box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
		pointer-events: none;
	}
	/* Don't bleed up into the sender-name header above the media. */
	.sender-name + .media.photos {
		margin-top: 0;
	}
	/* Leave a gap before a caption below the media instead of bleeding down. */
	.media.photos:has(+ .caption) {
		margin-bottom: 4px;
	}
	/* Leave a gap before a caption below the file. */
	.media.file:has(+ .caption) {
		margin-bottom: 4px;
	}
	/* Space the file row away from the sender-name header above it in groups. */
	.sender-name + .media.file {
		margin-top: 6px;
	}

	/* Image-only bubbles bleed to the bottom edge with the timestamp overlaid
	 * on a gradient scrim, matching Signal. */
	.photo-meta {
		color: rgba(255, 255, 255, 0.95);
		background: linear-gradient(to top, rgba(0, 0, 0, 0.45), transparent);
	}

	.photo-meta :global(.quiet),
	.photo-meta :global(.dark-quiet) {
		color: rgba(255, 255, 255, 0.95);
	}
</style>
