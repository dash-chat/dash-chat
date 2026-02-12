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

	function sendReaction(hash: string, emoji: string | null) {}

	function toggleEmoji(m: any, d: any) {
		return '';
	}
</script>

<Card raised class={`${classes} message`}>
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
	{#each condenseReactions(message.reactions, deviceId) as reaction}
		<Chip class={(reaction.own ? 'border' : '') + 'px-1 py-0 mr-1'}>
			{reaction.emoji}{#if reaction.count > 1}{reaction.count}{/if}
		</Chip>
	{/each}
</Card>
