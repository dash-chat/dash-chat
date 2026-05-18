import { type ReactivePromise, reactive, relay } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import type { TrackedMailboxesClient } from './tracked-mailboxes-client';
import {
	type MailboxConnectionState,
	type MailboxId,
	type MailboxSyncState,
	type SyncStateEntry,
	syncKey,
} from './types';

export class TrackedMailboxesStore {
	constructor(public client: TrackedMailboxesClient) {}

	mailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state => {
				const unsub = this.client.subscribeTrackedMailboxIds(ids => {
					state.value = ids;
				});
				return unsub;
			}),
	);

	connectionState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxConnectionState> =>
			relay<MailboxConnectionState>(state => {
				const unsub = this.client.subscribeConnectionState(mailboxId, s => {
					state.value = s;
				});
				return unsub;
			}),
	);

	syncState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxSyncState> =>
			relay<MailboxSyncState>(state => {
				const unsub = this.client.subscribeSyncState(mailboxId, entries => {
					state.value = entriesToMap(entries);
				});
				return unsub;
			}),
	);

	/// Per-(topic, author) view across all known mailboxes. Recomputes when
	/// `mailboxIds` or any per-mailbox `syncState` changes.
	syncStateForLog = reactive(
		async (
			topicId: TopicId,
			author: DeviceId,
		): Promise<Map<MailboxId, number>> => {
			const ids = await this.mailboxIds();
			const out = new Map<MailboxId, number>();
			const key = syncKey(topicId, author);
			for (const id of ids) {
				const sync = await this.syncState(id);
				const seq = sync.get(key);
				if (seq !== undefined) {
					out.set(id, seq);
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
			const out: MailboxId[] = [];
			map.forEach((mailboxSeq, mailboxId) => {
				if (mailboxSeq >= seq) out.push(mailboxId);
			});
			return out;
		},
	);
}

function entriesToMap(entries: SyncStateEntry[]): MailboxSyncState {
	const map: MailboxSyncState = new Map();
	for (const e of entries) {
		map.set(syncKey(e.topic_id, e.author), e.seq_num);
	}
	return map;
}
