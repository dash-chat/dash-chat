import { type ReactivePromise, reactive, relay } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import type {
	TrackedMailboxesClient,
	UnsubscribeFn,
} from './tracked-mailboxes-client';
import type {
	MailboxConnectionState,
	MailboxId,
	MailboxSyncState,
} from './types';

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

export class TrackedMailboxesStore {
	constructor(public client: TrackedMailboxesClient) {}

	mailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state =>
				bridgeUnsub(
					this.client.subscribeTrackedMailboxIds(ids => {
						state.value = ids;
					}),
				),
			),
	);

	connectionState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxConnectionState> =>
			relay<MailboxConnectionState>(state =>
				bridgeUnsub(
					this.client.subscribeConnectionState(mailboxId, s => {
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

	/// Per-(topic, author) view across all known mailboxes. Recomputes when
	/// `mailboxIds` or any per-mailbox `syncState` changes.
	syncStateForLog = reactive(
		async (
			topicId: TopicId,
			author: DeviceId,
		): Promise<Map<MailboxId, number>> => {
			const ids = await this.mailboxIds();
			const out = new Map<MailboxId, number>();
			for (const id of ids) {
				const sync = await this.syncState(id);
				const seq = sync[topicId]?.[author];
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
