import { ReactivePromise, reactive } from 'signalium';

import type { DeviceId, TopicId } from '../p2panda/types';
import { subscribeChannel } from '../utils/tauri-channel';
import {
	type MailboxConnectionState,
	type MailboxId,
	type MailboxSyncState,
} from './types';

// Flip the UI to "disconnected" after this many consecutive errors. Intentionally
// lower than the backend's degraded_threshold (5) so the UI reacts faster than
// the connection state alone would suggest.
const UI_DISCONNECTED_ERROR_THRESHOLD = 2;

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
	activeMailboxIds = reactive(() =>
		subscribeChannel<MailboxId[]>('mailbox_subscribe_active_ids'),
	);

	allMailboxIds = reactive(() =>
		subscribeChannel<MailboxId[]>('mailbox_subscribe_all_ids'),
	);

	connectionState = reactive((mailboxId: MailboxId) =>
		subscribeChannel<MailboxConnectionState>(
			'mailbox_subscribe_connection_state',
			{ mailboxId },
		),
	);

	cloudMailboxId = reactive(() =>
		subscribeChannel<MailboxId | null>('mailbox_subscribe_cloud_id'),
	);

	connectionStatus = reactive(async () => {
		const activeMailboxIds = await this.activeMailboxIds();

		const mailboxesConnectionStates = await ReactivePromise.all(
			activeMailboxIds.map(mailboxId => this.connectionState(mailboxId)),
		);

		const cloudId = await this.cloudMailboxId();
		const cloudMailboxIndex = activeMailboxIds.findIndex(
			mailboxId => mailboxId === cloudId,
		);

		const connectedToCloudMailboxServer =
			cloudMailboxIndex >= 0 &&
			mailboxesConnectionStates[cloudMailboxIndex].status === 'Active' &&
			!!mailboxesConnectionStates[cloudMailboxIndex].last_success_at &&
			mailboxesConnectionStates[cloudMailboxIndex].consecutive_errors <
				UI_DISCONNECTED_ERROR_THRESHOLD;

		let connectedLocalMailboxCount = 0;

		for (let i = 0; i < activeMailboxIds.length; i++) {
			if (i === cloudMailboxIndex) continue;

			const connectionState = mailboxesConnectionStates[i];
			if (
				connectionState.status === 'Active' &&
				!!connectionState.last_success_at &&
				connectionState.consecutive_errors < UI_DISCONNECTED_ERROR_THRESHOLD
			)
				connectedLocalMailboxCount++;
		}

		return {
			connectedToCloudMailboxServer,
			connectedLocalMailboxCount,
		};
	});

	syncState = reactive((mailboxId: MailboxId) =>
		subscribeChannel<MailboxSyncState>('mailbox_subscribe_sync_state', {
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
