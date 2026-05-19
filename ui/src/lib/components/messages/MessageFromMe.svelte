<script lang="ts">
	import { Card } from 'konsta/svelte';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import {
		highlightMatch,
		myMessageCornerClasses,
		type MessagePosition,
	} from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import Reactions from './Reactions.svelte';

	let {
		message,
		position,
		myDeviceId,
		searchQuery,
		onToggleReaction,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		searchQuery: string;
		onToggleReaction: (emoji: string) => void;
	} = $props();

	const cornerClasses = $derived(myMessageCornerClasses(position));
	const isLast = $derived(position === 'last' || position === 'single');
</script>

<Card
	raised
	contentWrapPadding="p-2"
	class={`m-0 min-w-0 [overflow-wrap:anywhere] bg-brand-primary text-white ${cornerClasses}`}
>
	<div class="row gap-2 mx-1" style="align-items: end">
		<span class="flex-1">
			{#if searchQuery}
				{@html highlightMatch(message.content, searchQuery)}
			{:else}
				{message.content}
			{/if}
		</span>

		{#if isLast}
			<MessageTimestamp timestamp={message.timestamp} class="dark-quiet" />
		{/if}
	</div>
</Card>
{#if Object.keys(message.reactions).length}
	<div class="flex -mt-1.5 mb-0.5 gap-0.5 px-1">
		<Reactions reactions={message.reactions} {myDeviceId} {onToggleReaction} />
	</div>
{/if}
