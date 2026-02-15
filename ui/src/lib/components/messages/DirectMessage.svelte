<script lang="ts">
	import {
		toPromise,
		type ChatsStore,
		type ContactCode,
		type ContactRequest,
		type ContactsStore,
		type DeviceId,
		type Message,
	} from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import {
		lessThanAMinuteAgo,
		moreThanAnHourAgo,
		moreThanAWeekAgo,
		moreThanAYearAgo,
	} from '$lib/utils/time';

	import { Card, Chip } from 'konsta/svelte';
	import { condenseReactions } from '$lib/utils/emojis';

	interface Props {
		message: Message;
		hash: string;
		isLastMessage: boolean;
		isOwnMessage: boolean;
		deviceId: DeviceId;
		classes: string;
		onclick: () => void;
	}

	let {
		message,
		hash,
		deviceId,
		classes,
		isLastMessage,
		isOwnMessage,
		onclick,
	}: Props = $props();

</script>

<div>
	<Card raised class={`${classes} message`} style="position: relative;">
		<div
			role="button"
			tabindex="0"
			{onclick}
			class="row gap-2 mx-1"
			style="align-items: center"
		>
			<span>{message.content}</span>
			{#if isLastMessage}
				<div class="{isOwnMessage ? 'dark-quiet' : 'quiet'} text-xs">
					{#if lessThanAMinuteAgo(message.timestamp)}
						<span>{m.now()}</span>
					{:else if moreThanAnHourAgo(message.timestamp)}
						<wa-format-date
							hour="numeric"
							minute="numeric"
							hour-format="24"
							date={new Date(message.timestamp)}
						></wa-format-date>
					{:else}
						<wa-relative-time
							sync
							format="narrow"
							date={new Date(message.timestamp)}
						>
						</wa-relative-time>
					{/if}
				</div>
			{/if}
		</div>
	</Card>
	{#if Object.values(message.reactions).length}
		<div class="h-4 relative">
			<div class="absolute -top-3 h-7 overflow-hidden px-1">
				{#each condenseReactions(message.reactions, deviceId) as reaction}
				<Chip class={(reaction.own ? '' : '') + ' h-6 px-2 mr-1 text-xs'}>
					{reaction.emoji}{#if reaction.count > 1}{reaction.count}{/if}
				</Chip>
				{/each}
			</div>
		</div>
	{/if}
</div>
