<script lang="ts">
	import {
		UNSENT_WINDOW_MS,
		displayDeliveryStatus,
		type MessageDeliveryStatus,
		type MessageDisplayStatus,
	} from 'dash-chat-stores';

	import { m } from '$lib/paraglide/messages.js';

	import deliveredSvg from '$lib/assets/message-status/delivered.svg?raw';
	import mailboxSvg from '$lib/assets/message-status/mailbox.svg?raw';
	import sendingSvg from '$lib/assets/message-status/sending.svg?raw';
	import unsentSvg from '$lib/assets/message-status/unsent.svg?raw';

	const icons: Record<MessageDisplayStatus, string> = {
		delivered: deliveredSvg,
		mailbox: mailboxSvg,
		sending: sendingSvg,
		unsent: unsentSvg,
	};

	const statusLabels: Record<MessageDisplayStatus, () => string> = {
		delivered: m.messageStatusDelivered,
		mailbox: m.messageStatusMailbox,
		sending: m.messageStatusSending,
		unsent: m.messageStatusUnsent,
	};

	interface Props {
		status: MessageDeliveryStatus;
		timestamp: number;
	}

	const props: Props = $props();

	let now = $state(Date.now());
	const displayStatus = $derived(
		displayDeliveryStatus(props.status, props.timestamp, now),
	);

	$effect(() => {
		if (displayStatus !== 'unsent') return;
		const timer = setTimeout(
			() => {
				now = Date.now();
			},
			props.timestamp + UNSENT_WINDOW_MS - now,
		);
		return () => clearTimeout(timer);
	});
</script>

<div
	data-testid="message-status"
	data-status={displayStatus}
	class="message-status"
	aria-label={statusLabels[displayStatus]()}
>
	{@html icons[displayStatus]}
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

	.message-status[data-status='unsent'] :global(svg) {
		animation: spin-cw 1.5s linear infinite;
	}

	@keyframes spin-cw {
		to {
			transform: rotate(360deg);
		}
	}
</style>
