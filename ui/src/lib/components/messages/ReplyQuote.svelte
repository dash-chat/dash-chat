<script lang="ts">
	import type { DeviceId, Hash, MessageReply } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import BlobImage from '$lib/components/BlobImage.svelte';

	let {
		reply,
		authorName = '',
		myDeviceId,
		mine = false,
		onNavigate,
	}: {
		reply: MessageReply;
		/** Display name of the quoted author. */
		authorName?: string;
		myDeviceId: DeviceId;
		/** Whether the enclosing bubble is my own message (picks the color scheme). */
		mine?: boolean;
		/** Scroll the chat to the quoted message. */
		onNavigate?: (target: Hash) => void;
	} = $props();

	const scrollTarget = $derived(
		reply.kind === 'deleted-for-me' ? undefined : reply.scrollTarget,
	);
	const thumbnail = $derived(
		reply.kind === 'content' && reply.media?.kind === 'photos'
			? reply.media.photos[0]
			: undefined,
	);

	/** The tombstone line for a deleted quote, naming who deleted it as the
	 * deleted message's own placeholder does. A delete-for-me is always ours. */
	const deletedText = $derived.by(() => {
		if (reply.kind === 'content') return '';
		if (reply.kind === 'deleted-for-me' || reply.author === myDeviceId) {
			return m.youDeletedThisMessage();
		}
		return m.someoneDeletedThisMessage({
			name: authorName || m.unknownSender(),
		});
	});

	function navigate() {
		if (scrollTarget !== undefined) onNavigate?.(scrollTarget);
	}
</script>

<button
	type="button"
	class="reply-quote {mine ? 'reply-quote-mine' : 'reply-quote-others'}"
	class:cursor-default={scrollTarget === undefined}
	onclick={navigate}
	data-testid="reply-quote"
>
	<span class="reply-quote-bar"></span>
	<span class="flex min-w-0 flex-1 flex-col items-start gap-0.5 px-2 py-1.5">
		{#if reply.kind === 'content'}
			{#if authorName}
				<span class="reply-quote-author">{authorName}</span>
			{/if}
			<span class="reply-quote-text" data-testid="reply-quote-text">
				{#if reply.text}
					{reply.text}
				{:else if reply.media?.kind === 'photos'}
					{m.photo()}
				{:else if reply.media?.kind === 'file'}
					{reply.media.file.name}
				{:else if reply.media?.kind === 'voice_note'}
					{m.voiceMessage()}
				{/if}
			</span>
		{:else}
			<span
				class="reply-quote-text flex items-center gap-1 italic"
				data-testid="reply-quote-deleted"
			>
				{deletedText}
			</span>
		{/if}
	</span>
	{#if thumbnail}
		<span class="reply-quote-thumb relative shrink-0">
			<BlobImage
				item={thumbnail}
				alt={m.photo()}
				imgClass="h-full w-full object-cover"
			/>
		</span>
	{/if}
</button>

<style>
	.reply-quote {
		display: flex;
		align-items: stretch;
		width: 100%;
		min-width: 0;
		/* A <button>'s intrinsic block size does not follow its flex content, so
		   the author and text lines get shrunk and clipped without this. */
		height: fit-content;
		margin-bottom: 0.25rem;
		border-radius: 0.5rem;
		overflow: hidden;
		text-align: start;
		font-size: 0.875rem;
	}

	.reply-quote-bar {
		flex-shrink: 0;
		width: 4px;
	}

	.reply-quote-author {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 600;
		font-size: 0.8125rem;
	}

	.reply-quote-text {
		max-width: 100%;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		overflow-wrap: anywhere;
	}

	.reply-quote-thumb {
		width: 3rem;
		min-height: 3rem;
	}

	/* Inside my (brand-colored) bubble: translucent white over the brand color. */
	.reply-quote-mine {
		background-color: rgba(255, 255, 255, 0.18);
		color: white;
	}
	.reply-quote-mine .reply-quote-bar {
		background-color: rgba(255, 255, 255, 0.85);
	}

	/* Inside a peer's (surface-colored) bubble: subtle tint + brand accent bar. */
	.reply-quote-others {
		background-color: rgba(0, 0, 0, 0.06);
	}
	:global(.dark) .reply-quote-others {
		background-color: rgba(255, 255, 255, 0.08);
	}
	.reply-quote-others .reply-quote-bar {
		background-color: var(--color-brand-primary);
	}
	.reply-quote-others .reply-quote-author {
		color: var(--color-brand-primary);
	}
</style>
