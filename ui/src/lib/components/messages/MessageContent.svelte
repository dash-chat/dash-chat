<script lang="ts">
	import type { Snippet } from 'svelte';
	import { type Message, hasBody } from 'dash-chat-stores';
	import { senderColor } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';
	import PhotosAttachment from './attachments/PhotosAttachment.svelte';
	import FileAttachment from './attachments/FileAttachment.svelte';
	import VoiceNoteAttachment from './attachments/voice-notes/VoiceNoteAttachment.svelte';
	import MessageText from './MessageText.svelte';

	let {
		message,
		searchQuery,
		metadata,
		senderName = '',
		showSenderName = false,
	}: {
		message: Message;
		searchQuery: string;
		metadata?: Snippet;
		/** Author display name; shown as the bubble header (in groups) and used
		 * as the lightbox title. */
		senderName?: string;
		showSenderName?: boolean;
	} = $props();

	const body = $derived(hasBody(message.content) ? message.content : null);
	const media = $derived(body?.media ?? null);
	const voiceNote = $derived(media?.find(m => m.kind === 'VoiceNote'));
	const file = $derived(media?.find(m => m.kind === 'File'));
	const photos = $derived(
		file ? [] : (media?.filter(m => m.kind === 'Photo') ?? []),
	);
	const hasText = $derived(!!body?.message);
	const isMediaOnly = $derived(
		!hasText && (photos.length > 0 || !!file || !!voiceNote),
	);

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
{#if photos.length > 0}
	<div class="media photos">
		<PhotosAttachment {photos} {senderName} timestamp={message.timestamp} />
		{#if isMediaOnly && metadata}
			<div
				class="photo-meta pointer-events-none absolute inset-x-0 bottom-0 flex items-center justify-end gap-1 px-2 pt-4 pb-1"
			>
				{@render metadata()}
			</div>
		{/if}
	</div>
{:else if file}
	<div class="media file">
		<FileAttachment {file} metadata={isMediaOnly ? metadata : undefined} />
	</div>
{:else if voiceNote}
	<div class="media voice">
		<VoiceNoteAttachment
			voice={voiceNote}
			metadata={isMediaOnly ? metadata : undefined}
		/>
	</div>
{/if}
{#if hasText || (metadata && !isMediaOnly)}
	<div class="caption relative px-1">
		{#if metadata}
			<div
				class="absolute bottom-0 end-0 flex items-center gap-1 whitespace-nowrap select-none"
				bind:clientWidth={metadataWidth}
			>
				{@render metadata()}
			</div>
		{/if}
		<div class="max-w-full" use:shrinkToWidestLine>
			<MessageText text={body?.message ?? ''} {searchQuery} />
			<!-- Reserves the metadata's space in the bottom-end corner, since
			     wrapped text cannot be made to avoid an absolute box via CSS. -->
			{#if metadata}
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
	/* Leave a gap before a caption below the file/voice row. */
	.media.file:has(+ .caption),
	.media.voice:has(+ .caption) {
		margin-bottom: 4px;
	}
	/* Space the file/voice row away from the sender-name header in groups. */
	.sender-name + .media.file,
	.sender-name + .media.voice {
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
