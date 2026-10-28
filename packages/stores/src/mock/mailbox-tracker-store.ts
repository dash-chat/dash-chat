import { ReactivePromise, reactive, relay } from 'signalium';

import type {
	AllMailboxesConnectionState,
	IMailboxTrackerStore,
} from '../mailbox-tracker/mailbox-tracker-store';
import {
	type MailboxConnectionHealth,
	type MailboxId,
	type OperationSyncState,
} from '../mailbox-tracker/types';
import type { DeviceId, TopicId } from '../p2panda/types';

const MOCK_MAILBOX_ID: MailboxId = 'mock-mailbox';

const syncedToInfinity: OperationSyncState = new Proxy({} as OperationSyncState, {
	get: () =>
		new Proxy(
			{},
			{
				get: () => Number.MAX_SAFE_INTEGER,
			},
		),
});

const constant = <T>(value: T): ReactivePromise<T> =>
	relay<T>(state => {
		state.value = value;
		return () => {};
	});

export class MockMailboxTrackerStore implements IMailboxTrackerStore {
	activeMailboxIds = reactive(() => constant<MailboxId[]>([MOCK_MAILBOX_ID]));

	allMailboxIds = reactive(() => constant<MailboxId[]>([MOCK_MAILBOX_ID]));

	connectionState = reactive((_mailboxId: MailboxId) =>
		constant<MailboxConnectionHealth>({
			status: 'Active',
			consecutive_errors: 0,
			next_poll_in_ms: 5000,
			last_success_at: new Date().toISOString(),
			last_error: null,
		}),
	);

	syncState = reactive((_mailboxId: MailboxId) => constant(syncedToInfinity));

	syncStateForLog = reactive(
		async (
			_topicId: TopicId,
			_author: DeviceId,
		): Promise<Record<MailboxId, number>> => ({
			[MOCK_MAILBOX_ID]: Number.MAX_SAFE_INTEGER,
		}),
	);

	syncedMailboxesForOp = reactive(
		async (
			_topicId: TopicId,
			_author: DeviceId,
			_seq: number,
		): Promise<MailboxId[]> => [MOCK_MAILBOX_ID],
	);

	connectionStatusForAllMailboxes = reactive(
		async (): Promise<AllMailboxesConnectionState> => ({
			cloudMailbox: {
				status: 'Active',
				consecutive_errors: 0,
				next_poll_in_ms: 5000,
				// Far future: this value is computed once, and a success before the
				// page's last resume would read as unproven rather than connected.
				last_success_at: '9999-01-01T00:00:00Z',
				last_error: null,
			},
			localMailboxes: [],
		}),
	);

	syncStatusForOp = reactive(
		async (_topicId: TopicId, _author: DeviceId, _seq: number) => ({
			syncedWithCloudMailbox: true,
			syncedWithAnyLocalMailbox: false,
		}),
	);
}
