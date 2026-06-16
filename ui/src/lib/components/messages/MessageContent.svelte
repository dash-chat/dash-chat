<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Message } from 'dash-chat-stores';
	import { highlightMatch, senderColor } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';
	import MessageAttachment from './MessageAttachment.svelte';

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

	const media = $derived(message.content.media);
	const hasText = $derived(!!message.content.message);
	const isPhotoOnly = $derived(media?.kind === 'photos' && !hasText);

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
{#if media}
	<div
		class="media"
		class:photos={media.kind === 'photos'}
		class:file={media.kind === 'file'}
	>
		<MessageAttachment {media} {senderName} timestamp={message.timestamp} />
		{#if isPhotoOnly && metadata}
			<div class="photo-meta">{@render metadata()}</div>
		{/if}
	</div>
{/if}
{#if hasText || (metadata && !isPhotoOnly)}
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
			<span class="whitespace-pre-wrap">
				{#if searchQuery}
					{@html highlightMatch(message.content.message, searchQuery)}
				{:else}
					{message.content.message}
				{/if}
			</span>
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
	.media.file {
		margin: -4px 0 6px;
	}

	/* Image-only bubbles bleed to the bottom edge with the timestamp overlaid
	 * on a gradient scrim, matching Signal. */
	.photo-meta {
		position: absolute;
		inset-inline: 0;
		bottom: 0;
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 4px;
		padding: 16px 8px 4px;
		color: rgba(255, 255, 255, 0.95);
		background: linear-gradient(to top, rgba(0, 0, 0, 0.45), transparent);
		pointer-events: none;
	}

	.photo-meta :global(.quiet),
	.photo-meta :global(.dark-quiet) {
		color: rgba(255, 255, 255, 0.95);
	}
</style>
