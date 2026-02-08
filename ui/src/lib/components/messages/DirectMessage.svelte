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

	import {
		Card,
	} from 'konsta/svelte';


	interface Props {
		message: Message;
        hash: string;
        isLastMessage: boolean;
        isOwnMessage: boolean;
		classes: string;
	}

	let { message, hash, classes, isLastMessage, isOwnMessage}: Props = $props();

    function sendReaction(hash: string, emoji: string|null) {

    }

    function toggleEmoji(m:any, d: any) {
        return ''
    }

    const myDeviceId = ''
</script>

<Card raised class={`${classes} message`}>
	<div
		role="button"
		tabindex="0"
		on:click={() =>
			sendReaction(hash, toggleEmoji(message.reactions, myDeviceId))}
		on:keydown={e => console.log('ok')}
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
	<div>
		{Object.values(message.reactions).join('')}
	</div>
</Card>
