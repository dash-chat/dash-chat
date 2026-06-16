<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Message } from 'dash-chat-stores';
	import { highlightMatch } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';
	import MessageAttachment from './MessageAttachment.svelte';

	let {
		message,
		searchQuery,
		metadata,
		senderName = '',
		withContentAbove = false,
	}: {
		message: Message;
		searchQuery: string;
		metadata?: Snippet;
		senderName?: string;
		withContentAbove?: boolean;
	} = $props();

	let metadataWidth = $state(0);
</script>

{#if message.content.media}
	<MessageAttachment
		media={message.content.media}
		{withContentAbove}
		withContentBelow={!!message.content.message || !!metadata}
		{senderName}
		timestamp={message.timestamp}
	/>
{/if}
{#if message.content.message || metadata}
	<div class="relative px-1">
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
