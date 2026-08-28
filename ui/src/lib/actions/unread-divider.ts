import type { DeviceId, Hash } from 'dash-chat-stores';

export interface UnreadDividerInfo {
	hash: Hash | null;
	count: number;
}

interface DividerEvent {
	kind: string;
	message?: { author: DeviceId; timestamp: number };
}

interface DividerDay {
	eventsGroups: ReadonlyArray<ReadonlyArray<readonly [Hash, DividerEvent]>>;
}

/** Tracks the "N unread messages" divider of a chat page.
 *
 * The divider position is captured at the first unread peer message and then
 * stays sticky, so it doesn't shift as messages get marked read. Messages
 * sent before the page was entered (the backlog) always get the divider;
 * messages arriving while the user sits at the bottom don't — those are
 * auto-read on sight. The message list can hydrate over several renders after
 * mount, so `compute` keeps trying to capture until a divider is found; the
 * entry timestamp is what distinguishes backlog from live arrivals. */
export function createUnreadDividerTracker() {
	const enteredAt = Date.now();
	let capturedUnreadHash: Hash | null = null;

	function compute(
		days: readonly DividerDay[] | undefined,
		readHashes: Set<Hash> | undefined,
		deviceId: DeviceId | undefined,
		isAtBottom: boolean,
	): UnreadDividerInfo {
		if (!days || !readHashes || !deviceId) {
			return { hash: null, count: 0 };
		}

		const entries = days.flatMap(day => day.eventsGroups).flat();
		const isPeerMessage = ([, item]: readonly [Hash, DividerEvent]) =>
			item.kind === 'message' &&
			item.message !== undefined &&
			item.message.author !== deviceId;

		if (capturedUnreadHash === null) {
			// The first unread decides: anything after it is even newer.
			const first = entries.find(
				entry => isPeerMessage(entry) && !readHashes.has(entry[0]),
			);
			if (first && (first[1].message!.timestamp <= enteredAt || !isAtBottom)) {
				capturedUnreadHash = first[0];
			}
		}

		if (capturedUnreadHash === null) return { hash: null, count: 0 };

		// Count all peer messages from the divider position onwards. This is
		// stable when messages are marked as read (count doesn't drop) and
		// increases when new messages arrive.
		const start = entries.findIndex(([hash]) => hash === capturedUnreadHash);
		const count =
			start === -1 ? 0 : entries.slice(start).filter(isPeerMessage).length;

		return { hash: capturedUnreadHash, count };
	}

	/** Forget the captured divider (e.g. after the user sends a message). */
	function reset() {
		capturedUnreadHash = null;
	}

	return { compute, reset };
}
