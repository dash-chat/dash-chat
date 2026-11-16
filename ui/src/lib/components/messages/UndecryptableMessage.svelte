<script lang="ts">
	import { mdiLockAlertOutline } from '@mdi/js';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import type { MessagePosition } from './message-helpers';
	import MessagePlaceholder from './MessagePlaceholder.svelte';

	let {
		message,
		position,
		myDeviceId,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
	} = $props();
</script>

<MessagePlaceholder
	{message}
	{position}
	{myDeviceId}
	icon={mdiLockAlertOutline}
	text={m.messageNotDecrypted()}
	testId="message-undecryptable-placeholder"
/>
