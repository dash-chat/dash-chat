<script lang="ts">
	import { type MessageDeliveryStatus } from 'dash-chat-stores';

	import { m } from '$lib/paraglide/messages.js';

	import deliveredSvg from '$lib/assets/message-status/delivered.svg?raw';
	import mailboxSvg from '$lib/assets/message-status/mailbox.svg?raw';
	import sendingSvg from '$lib/assets/message-status/sending.svg?raw';

	const icons: Record<MessageDeliveryStatus, string> = {
		delivered: deliveredSvg,
		mailbox: mailboxSvg,
		sending: sendingSvg,
	};

	const statusLabels: Record<MessageDeliveryStatus, () => string> = {
		delivered: m.messageStatusDelivered,
		mailbox: m.messageStatusMailbox,
		sending: m.messageStatusSending,
	};

	interface Props {
		status: MessageDeliveryStatus;
	}

	const props: Props = $props();
</script>

<div
	data-testid="message-status"
	data-status={props.status}
	class="message-status"
	aria-label={statusLabels[props.status]()}
>
	{@html icons[props.status]}
</div>

<style>
	.message-status :global(svg) {
		display: block;
		width: auto;
		height: 0.875rem;
	}

	.message-status :global(svg [stroke]) {
		stroke: currentColor;
	}
</style>
