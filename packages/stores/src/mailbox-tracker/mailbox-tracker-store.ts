import { Channel, invoke } from '@tauri-apps/api/core';
import { ReactivePromise, reactive, relay } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import { unregisterChannel } from '../utils/tauri-channel';
import type {
	MailboxConnectionState,
	MailboxId,
	MailboxSyncState,
} from './types';

export interface IMailboxTrackerStore {
	activeMailboxIds(): ReactivePromise<MailboxId[]>;
	allMailboxIds(): ReactivePromise<MailboxId[]>;
	connectionState(
		mailboxId: MailboxId,
	): ReactivePromise<MailboxConnectionState>;
	syncState(mailboxId: MailboxId): ReactivePromise<MailboxSyncState>;
	syncStateForLog(
		topicId: TopicId,
		author: DeviceId,
	): ReactivePromise<Record<MailboxId, number>>;
	syncedMailboxesForOp(
		topicId: TopicId,
		author: DeviceId,
		seq: number,
	): ReactivePromise<MailboxId[]>;
}

export class MailboxTrackerStore implements IMailboxTrackerStore {
	activeMailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state => {
				const channel = new Channel<MailboxId[]>();
				channel.onmessage = v => {
					state.value = v;
				};
				invoke('mailbox_subscribe_active_ids', { onEvent: channel });
				return () => unregisterChannel(channel);
			}),
	);

	allMailboxIds = reactive(
		(): ReactivePromise<MailboxId[]> =>
			relay<MailboxId[]>(state => {
				const channel = new Channel<MailboxId[]>();
				channel.onmessage = v => {
					state.value = v;
				};
				invoke('mailbox_subscribe_all_ids', { onEvent: channel });
				return () => unregisterChannel(channel);
			}),
	);

	connectionState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxConnectionState> =>
			relay<MailboxConnectionState>(state => {
				const channel = new Channel<MailboxConnectionState>();
				channel.onmessage = v => {
					state.value = v;
				};
				invoke('mailbox_subscribe_connection_state', {
					onEvent: channel,
					mailboxId,
				});
				return () => unregisterChannel(channel);
			}),
	);

	syncState = reactive(
		(mailboxId: MailboxId): ReactivePromise<MailboxSyncState> =>
			relay<MailboxSyncState>(state => {
				const channel = new Channel<MailboxSyncState>();
				channel.onmessage = v => {
					state.value = v;
				};
				invoke('mailbox_subscribe_sync_state', {
					onEvent: channel,
					mailboxId,
				});
				return () => unregisterChannel(channel);
			}),
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
