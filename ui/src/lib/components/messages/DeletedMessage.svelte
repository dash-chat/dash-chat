<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiCloseCircleOutline } from '@mdi/js';
	import { Card } from 'konsta/svelte';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import type { MessagePosition } from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';

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
	const sideClass = $derived(mine ? 'outgoing-message' : 'incoming-message');
	const isLast = $derived(position === 'last' || position === 'single');
</script>

<Card
	outline
	contentWrapPadding="p-2"
	colors={{
		bgIos: 'bg-transparent',
		bgMaterial: 'bg-transparent',
		outlineIos: 'border-current',
		outlineMaterial: 'border-current',
	}}
	class={`message quiet ${sideClass} ${position}-message`}
>
	<div class="flex flex-col gap-1">
		<span
			class="flex items-center gap-1"
			data-testid="message-deleted-placeholder"
		>
			<wa-icon
				class="small-icon shrink-0"
				src={wrapPathInSvg(mdiCloseCircleOutline)}
			></wa-icon>
			{mine
				? m.youDeletedThisMessage()
				: m.someoneDeletedThisMessage({
						name: senderName ?? m.unknownSender(),
					})}
		</span>
		{#if isLast}
			<MessageTimestamp timestamp={message.timestamp} class="self-end" />
		{/if}
	</div>
</Card>
