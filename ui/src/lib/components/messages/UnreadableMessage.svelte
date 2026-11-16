<script lang="ts">
	import { mdiLockAlertOutline, mdiLockClock } from '@mdi/js';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import {
		type MessagePosition,
		messagePlaceholderText,
	} from './message-helpers';
	import MessagePlaceholder from './MessagePlaceholder.svelte';

	let {
		message,
		reason,
		position,
		myDeviceId,
	}: {
		message: Message;
		reason: 'awaiting-decryption' | 'undecryptable';
		position: MessagePosition;
		myDeviceId: DeviceId;
	} = $props();

	const waiting = $derived(reason === 'awaiting-decryption');
</script>

<MessagePlaceholder
	{message}
	{position}
	{myDeviceId}
	icon={waiting ? mdiLockClock : mdiLockAlertOutline}
	text={messagePlaceholderText(reason)}
	testId={waiting
		? 'message-awaiting-decryption-placeholder'
		: 'message-undecryptable-placeholder'}
/>
