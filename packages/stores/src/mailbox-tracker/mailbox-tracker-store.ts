import { reactive } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import { buildReactiveChannel } from '../utils/tauri-channel';
import type { MailboxTrackerClient } from './mailbox-tracker-client';
import type { MailboxId } from './types';

export class MailboxTrackerStore {
	constructor(public client: MailboxTrackerClient) {}

	activeMailboxIds = buildReactiveChannel(
		this.client.subscribeActiveMailboxIds,
	);

	allMailboxIds = buildReactiveChannel(this.client.subscribeAllMailboxIds);

	connectionState = buildReactiveChannel(this.client.subscribeConnectionState);

	syncState = buildReactiveChannel(this.client.subscribeSyncState);

	/// Per-(topic, author) view across every mailbox we've ever synced with.
	/// Recomputes when `allMailboxIds` or any per-mailbox `syncState` changes.
	syncStateForLog = reactive(
		async (
			topicId: TopicId,
			author: DeviceId,
		): Promise<Record<MailboxId, number>> => {
			const ids = await this.allMailboxIds();
			const out: Record<MailboxId, number> = {};
			for (const id of ids) {
				const sync = await this.syncState(id);
				const seq = sync[topicId]?.[author];
				if (seq !== undefined) {
					out[id] = seq;
				}
			}
			return out;
		},
	);

	/// IDs of mailboxes that have synced at least up to `seq` for the (topic, author) log.
	syncedMailboxesForOp = reactive(
		async (
			topicId: TopicId,
			author: DeviceId,
			seq: number,
		): Promise<MailboxId[]> => {
			const map = await this.syncStateForLog(topicId, author);
			return Object.entries(map)
				.filter(([, mailboxSeq]) => mailboxSeq >= seq)
				.map(([mailboxId]) => mailboxId);
		},
	);
}
