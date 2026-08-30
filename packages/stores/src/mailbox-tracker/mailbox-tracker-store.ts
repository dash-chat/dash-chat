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
const UI_DISCONNECTED_ERROR_THRESHOLD = 3;

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

	// Registration itself proves reachability: the cloud mailbox is only registered
	// after a successful `/health` fetch, mDNS-discovered local ones after a port
	// probe, and local ones are unregistered once their announcement goes away. So
	// presence in `activeMailboxIds` plus a low error count is enough. Requiring a
	// recorded success on top would report a freshly rebuilt node as disconnected
	// until its first poll — iOS tears the node down on background and rebuilds it
	// on foreground, so that is every single foreground.
	//
	// `cloudLastFailureAtMs` lets a consumer tell a failure it just watched happen
	// from one inherited from a period it wasn't rendering. That distinction is
	// the whole game: Android denies network to backgrounded apps, so those polls
	// fail whatever is waiting on the other side, and reporting them on resume is
	// how a verdict about a connection the user never had reaches the screen.
	// Deciding what counts as recent needs to know when the UI resumed painting,
	// which only the UI knows — so this reports the fact and leaves the judgement
	// to the caller.
	connectionStatus = reactive(async () => {
		const activeMailboxIds = await this.activeMailboxIds();

		const mailboxesConnectionStates = await ReactivePromise.all(
			activeMailboxIds.map(mailboxId => this.connectionState(mailboxId)),
		);

		const cloudId = await this.cloudMailboxId();
		const cloudMailboxIndex = activeMailboxIds.findIndex(
			mailboxId => mailboxId === cloudId,
		);

		const cloudState =
			cloudMailboxIndex >= 0
				? mailboxesConnectionStates[cloudMailboxIndex]
				: undefined;

		const connectedToCloudMailboxServer =
			cloudState !== undefined &&
			cloudState.status === 'Active' &&
			cloudState.consecutive_errors < UI_DISCONNECTED_ERROR_THRESHOLD;

		const cloudLastFailure = cloudState?.last_error?.at;
		const cloudLastFailureAtMs =
			cloudLastFailure === undefined || cloudLastFailure === null
				? null
				: Date.parse(cloudLastFailure);

		let connectedLocalMailboxCount = 0;

		for (let i = 0; i < activeMailboxIds.length; i++) {
			if (i === cloudMailboxIndex) continue;

			const connectionState = mailboxesConnectionStates[i];
			if (
				connectionState.status === 'Active' &&
				connectionState.consecutive_errors < UI_DISCONNECTED_ERROR_THRESHOLD
			)
				connectedLocalMailboxCount++;
		}

		return {
			connectedToCloudMailboxServer,
			cloudLastFailureAtMs,
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
