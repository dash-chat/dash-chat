import { ReactivePromise, reactive } from 'signalium';
import { subscribe } from 'tauri-plugin-subscriptions';

import type { DeviceId, TopicId } from '../p2panda/types';
import {
	type MailboxConnectionHealth,
	type MailboxId,
	type OperationSyncState,
} from './types';

export interface AllMailboxesConnectionState {
	/** Null while the cloud mailbox is not registered. */
	cloudMailbox: MailboxConnectionHealth | null;
	localMailboxes: MailboxConnectionHealth[];
}

export interface IMailboxTrackerStore {
	activeMailboxIds(): ReactivePromise<MailboxId[]>;
	allMailboxIds(): ReactivePromise<MailboxId[]>;
	connectionState(
		mailboxId: MailboxId,
	): ReactivePromise<MailboxConnectionHealth>;
	connectionStatusForAllMailboxes(): ReactivePromise<AllMailboxesConnectionState>;
	syncState(mailboxId: MailboxId): ReactivePromise<OperationSyncState>;
	syncStateForLog(
		topicId: TopicId,
		author: DeviceId,
	): ReactivePromise<Record<MailboxId, number>>;
	syncedMailboxesForOp(
		topicId: TopicId,
		author: DeviceId,
		seq: number,
	): ReactivePromise<MailboxId[]>;
	syncStatusForOp(
		topicId: TopicId,
		author: DeviceId,
		seq: number,
	): ReactivePromise<{
		syncedWithCloudMailbox: boolean;
		syncedWithAnyLocalMailbox: boolean;
	}>;
}

export class MailboxTrackerStore implements IMailboxTrackerStore {
	activeMailboxIds = reactive(() =>
		subscribe<MailboxId[]>('mailbox_subscribe_active_ids'),
	);

	allMailboxIds = reactive(() =>
		subscribe<MailboxId[]>('mailbox_subscribe_all_ids'),
	);

	connectionState = reactive((mailboxId: MailboxId) =>
		subscribe<MailboxConnectionHealth>('mailbox_subscribe_connection_state', {
			mailboxId,
		}),
	);

	cloudMailboxId = reactive(() =>
		subscribe<MailboxId | null>('mailbox_subscribe_cloud_id'),
	);

	// Registration itself proves reachability: the cloud mailbox is only registered
	// after a successful `/health` fetch, mDNS-discovered local ones after a port
	// probe, and local ones are unregistered once their announcement goes away. So
	// what is listed here is what the app currently syncs through; judging how
	// well each one is doing is left to the caller.
	connectionStatusForAllMailboxes = reactive(
		async (): Promise<AllMailboxesConnectionState> => {
			const activeMailboxIds = await this.activeMailboxIds();
			const states = await ReactivePromise.all(
				activeMailboxIds.map(mailboxId => this.connectionState(mailboxId)),
			);
			const cloudId = await this.cloudMailboxId();
			const cloudIndex =
				cloudId === null ? -1 : activeMailboxIds.indexOf(cloudId);
			return {
				cloudMailbox: cloudIndex >= 0 ? states[cloudIndex] : null,
				localMailboxes: states.filter((_, i) => i !== cloudIndex),
			};
		},
	);

	syncState = reactive((mailboxId: MailboxId) =>
		subscribe<OperationSyncState>('mailbox_subscribe_sync_state', {
			mailboxId,
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

	syncStatusForOp = reactive(
		async (topicId: TopicId, author: DeviceId, seq: number) => {
			const syncedMailboxes = await this.syncedMailboxesForOp(
				topicId,
				author,
				seq,
			);

			const cloudId = await this.cloudMailboxId();
			const localMailboxes = syncedMailboxes.filter(
				mailbox => mailbox !== cloudId,
			);

			const syncedWithCloudMailbox =
				cloudId != null && syncedMailboxes.includes(cloudId);
			const syncedWithAnyLocalMailbox = localMailboxes.length > 0;

			return {
				syncedWithCloudMailbox,
				syncedWithAnyLocalMailbox,
			};
		},
	);
}
