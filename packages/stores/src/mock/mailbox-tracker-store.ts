import { ReactivePromise, reactive, relay } from 'signalium';

import type { IMailboxTrackerStore } from '../mailbox-tracker/mailbox-tracker-store';
import {
	type MailboxConnectionState,
	type MailboxId,
} from '../mailbox-tracker/types';
import type { DeviceId, TopicId } from '../p2panda/types';

const MOCK_MAILBOX_ID: MailboxId = 'mock-mailbox';

const constant = <T>(value: T): ReactivePromise<T> =>
	relay<T>(state => {
		state.value = value;
		return () => {};
	});

export class MockMailboxTrackerStore implements IMailboxTrackerStore {
	activeMailboxIds = reactive(() => constant<MailboxId[]>([MOCK_MAILBOX_ID]));

	connectionState = reactive((_mailboxId: MailboxId) =>
		constant<MailboxConnectionState>({
			status: 'Active',
			consecutive_errors: 0,
			next_poll_in_ms: 5000,
			last_success_at: new Date().toISOString(),
			last_error: null,
		}),
	);

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

	connectionStatus = reactive(async () => ({
		connectedToCloudMailboxServer: true,
		cloudLastFailureAtMs: null,
		connectedLocalMailboxCount: 0,
	}));

	syncStatusForOp = reactive(
		async (_topicId: TopicId, _author: DeviceId, _seq: number) => ({
			syncedWithCloudMailbox: true,
			syncedWithAnyLocalMailbox: false,
		}),
	);
}
