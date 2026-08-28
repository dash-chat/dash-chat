<script lang="ts">
	import { getContext } from 'svelte';
	import {
		type ChatId,
		type DeviceId,
		type MessageAckStore,
		type MessageDeliveryStatus,
	} from 'dash-chat-stores';

	import { useReactivePromise } from '$lib/stores/use-signal';

	import deliveredSvg from '$lib/assets/message-status/delivered.svg?raw';
	import mailboxSvg from '$lib/assets/message-status/mailbox.svg?raw';
	import sendingSvg from '$lib/assets/message-status/sending.svg?raw';

	const icons: Record<MessageDeliveryStatus, string> = {
		delivered: deliveredSvg,
		mailbox: mailboxSvg,
		sending: sendingSvg,
	};

	interface Props {
		chatId: ChatId;
		author: DeviceId;
		seq: number;
	}

	const props: Props = $props();

	const messageAckStore: MessageAckStore = getContext('message-acks-store');

	const status = useReactivePromise(
		messageAckStore.deliveryStatus,
		props.chatId,
		props.author,
		props.seq,
	);
</script>

{#await $status then status}
	<div
		data-testid="message-status"
		data-status={status}
		class="message-status"
		aria-label={status}
	>
		{@html icons[status]}
	</div>
{/await}

<style>
	.message-status {
		opacity: 1;
	}

	.message-status :global(svg) {
		display: block;
		width: auto;
		height: 0.875rem;
	}

	.message-status :global(svg [stroke]) {
		stroke: currentColor;
	}
</style>
