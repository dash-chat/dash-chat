<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Card } from 'konsta/svelte';
	import type { DeviceId, Message } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import type { MessagePosition } from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';

	let {
		message,
		position,
		myDeviceId,
		icon,
		text,
		testId,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		icon: string;
		text: string;
		testId: string;
	} = $props();

	const mine = $derived(message.author === myDeviceId);
	const sideClass = $derived(mine ? 'outgoing-message' : 'incoming-message');
	const isLast = $derived(position === 'last' || position === 'single');
</script>

<Card
	outline
	contentWrapPadding="px-2 py-(--message-padding-y)"
	colors={{
		bgIos: 'bg-transparent',
		bgMaterial: 'bg-transparent',
		outlineIos: 'border-current',
		outlineMaterial: 'border-current',
	}}
	class={`message quiet ${sideClass} ${position}-message`}
>
	<div class="flex flex-col gap-1">
		<span class="flex items-center gap-1" data-testid={testId}>
			<wa-icon class="small-icon shrink-0" src={wrapPathInSvg(icon)}></wa-icon>
			{text}
		</span>
		{#if isLast}
			<MessageTimestamp timestamp={message.timestamp} class="self-end" />
		{/if}
	</div>
</Card>
