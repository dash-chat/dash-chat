<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiCloseCircleOutline } from '@mdi/js';
	import { Card } from 'konsta/svelte';
	import type { Message } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import type { MessagePosition } from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';

	let {
		message,
		position,
		senderName,
	}: {
		message: Message;
		position: MessagePosition;
		/** The author's display name; omitted when the message is mine. */
		senderName?: string;
	} = $props();

	const sideClass = $derived(
		senderName === undefined ? 'outgoing-message' : 'incoming-message',
	);
</script>

<Card
	outline
	contentWrapPadding="px-2 py-2"
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
			{senderName === undefined
				? m.youDeletedThisMessage()
				: m.someoneDeletedThisMessage({ name: senderName })}
		</span>
		<MessageTimestamp timestamp={message.timestamp} class="self-end" />
	</div>
</Card>
