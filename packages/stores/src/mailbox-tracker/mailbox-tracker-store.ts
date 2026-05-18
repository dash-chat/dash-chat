import { type ReactivePromise, reactive, relay } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import type {
	MailboxTrackerClient,
	UnsubscribeFn,
} from './mailbox-tracker-client';
import type { MailboxId, MailboxSyncState, MailboxTracker } from './types';

function bridgeUnsub(pending: Promise<UnsubscribeFn>): UnsubscribeFn {
	let unsub: UnsubscribeFn | undefined;
	let cancelled = false;
	void pending.then(u => {
		if (cancelled) u();
		else unsub = u;
	});
	return () => {
		cancelled = true;
		unsub?.();
	};
}

export class MailboxTrackerStore {
	constructor(public client: MailboxTrackerClient) {}

	activeMailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state =>
				bridgeUnsub(
					this.client.subscribeActiveMailboxIds(ids => {
						state.value = ids;
					}),
				),
			),
	);

	allMailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state =>
				bridgeUnsub(
					this.client.subscribeAllMailboxIds(ids => {
						state.value = ids;
					}),
				),
			),
	);

	tracker = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxTracker> =>
			relay<MailboxTracker>(state =>
				bridgeUnsub(
					this.client.subscribeTracker(mailboxId, s => {
						state.value = s;
					}),
				),
			),
	);

	syncState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxSyncState> =>
			relay<MailboxSyncState>(state =>
				bridgeUnsub(
					this.client.subscribeSyncState(mailboxId, s => {
						state.value = s;
					}),
				),
			),
	);

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
