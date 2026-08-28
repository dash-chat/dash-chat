import { type ReactivePromise, reactive, relay } from 'signalium';

import type { TopicId } from '../p2panda/types';
import { MessageAcks } from '../types';
import { pollingRequired } from '../utils/polling-required';
import type { IMessageAckClient } from './message-ack-client';

const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED = pollingRequired();

export class MessageAckStore {
	constructor(public client: IMessageAckClient) {}

	/** Per author of `topic`, the highest operation acked by a device of
	 * another agent. A message is "delivered" when its seq is covered by the
	 * entry for its author. */
	acks = reactive(
		(topic: TopicId): ReactivePromise<MessageAcks> =>
			relay(state => {
				const fetchAcks = async () => {
					const acks = await this.client.getMessageAcks(topic);
					// Merge keeping the highest seq per author: an incremental
					// event can land before an older in-flight fetch resolves, and
					// a wholesale replace would let the stale result win.
					state.value = mergeAcks(state.value, acks);
				};

				fetchAcks();
				const interval = POLLING_ENABLED
					? setInterval(fetchAcks, POLL_INTERVAL_MS)
					: undefined;

				const unsub = this.client.onNewMessageAcks(topic, acks => {
					state.value = mergeAcks(state.value, acks);
				});

				return () => {
					clearInterval(interval);
					unsub();
				};
			}),
	);
}

function mergeAcks(
	current: MessageAcks | undefined,
	incoming: MessageAcks,
): MessageAcks {
	const merged: MessageAcks = { ...(current || {}) };
	for (const [author, acked] of Object.entries(incoming)) {
		const existing = merged[author];
		if (existing === undefined || acked.seq > existing.seq) {
			merged[author] = acked;
		}
	}
	return merged;
}
