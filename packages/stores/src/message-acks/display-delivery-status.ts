import { MessageDeliveryStatus } from '../types';

/** How long a not-yet-synced message shows as "unsent" (still trying) before
 * downgrading to "sending" (no connectivity). Covers the healthy worst case:
 * the peer-ack roundtrip is floored by the backend's 3s `message_ack_debounce`
 * (crates/dashchat-node/src/node.rs) plus sync latency both ways; the mailbox
 * path resolves well within 1s. If delivery is ever marked on direct peer
 * sync without the ack roundtrip, this can shrink to ~5s. */
export const UNSENT_WINDOW_MS = 60_000;

export type MessageDisplayStatus = MessageDeliveryStatus | 'unsent';

/** The status the UI should render for a message sent at `timestamp`:
 * `'unsent'` while a `'sending'` message is younger than `unsentWindowMs`,
 * the store status otherwise. */
export function displayDeliveryStatus(
	status: MessageDeliveryStatus,
	timestamp: number,
	now: number,
	unsentWindowMs: number = UNSENT_WINDOW_MS,
): MessageDisplayStatus {
	if (status === 'sending' && now - timestamp < unsentWindowMs) {
		return 'unsent';
	}
	return status;
}
