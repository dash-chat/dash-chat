<script lang="ts">
	import { mdiCloseCircleOutline } from '@mdi/js';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import type { MessagePosition } from './message-helpers';
	import MessagePlaceholder from './MessagePlaceholder.svelte';

	let {
		message,
		position,
		myDeviceId,
		senderName,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		senderName?: string;
	} = $props();

	const mine = $derived(message.author === myDeviceId);
</script>

<MessagePlaceholder
	{message}
	{position}
	{myDeviceId}
	icon={mdiCloseCircleOutline}
	text={mine
		? m.youDeletedThisMessage()
		: m.someoneDeletedThisMessage({
				name: senderName ?? m.unknownSender(),
			})}
	testId="message-deleted-placeholder"
/>
