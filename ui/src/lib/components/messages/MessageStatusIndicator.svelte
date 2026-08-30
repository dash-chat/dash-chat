<script lang="ts">
	import type { MessageDeliveryStatus } from 'dash-chat-stores';

	import { m } from '$lib/paraglide/messages.js';
	import { withinWindow } from '$lib/utils/time';

	import deliveredSvg from '$lib/assets/message-status/delivered.svg?raw';
	import mailboxSvg from '$lib/assets/message-status/mailbox.svg?raw';
	import sendingSvg from '$lib/assets/message-status/sending.svg?raw';
	import unsentSvg from '$lib/assets/message-status/unsent.svg?raw';

	/** How long a not-yet-synced message shows as "unsent" (still trying) before
	 * downgrading to "sending" (no connectivity). Covers the healthy worst case:
	 * the peer-ack roundtrip is floored by the backend's 3s `message_ack_debounce`
	 * (crates/dashchat-node/src/node.rs) plus sync latency both ways; the mailbox
	 * path resolves well within 1s. If delivery is ever marked on direct peer
	 * sync without the ack roundtrip, this can shrink to ~5s. */
	const UNSENT_WINDOW_MS = 60_000;

	type MessageDisplayStatus = MessageDeliveryStatus | 'unsent';

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

	// A 60s window can't be waited out in an e2e spec, so the binary under
	// test downgrades sooner — but stays well clear of the seconds a spec can
	// take to send a message and read the indicator back.
	const unsentWindowMs =
		import.meta.env.VITE_E2E === 'true' ? 15_000 : UNSENT_WINDOW_MS;

	const displayStatus: MessageDisplayStatus = $derived(
		props.status === 'sending' && withinWindow(props.timestamp, unsentWindowMs)
			? 'unsent'
			: props.status,
	);
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
