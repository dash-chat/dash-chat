import type { MailboxTrackerClient } from '../mailbox-tracker/mailbox-tracker-client';
import {
	type MailboxConnectionState,
	type MailboxId,
	type MailboxSyncState,
	PRODUCTION_MAILBOX_ID,
} from '../mailbox-tracker/types';
import type { UnsubscribeFn } from '../utils/tauri-channel';

const noop: UnsubscribeFn = () => {};

const syncedToInfinity: MailboxSyncState = new Proxy({} as MailboxSyncState, {
	get: () =>
		new Proxy(
			{},
			{
				get: () => Number.MAX_SAFE_INTEGER,
			},
		),
});

export class MockMailboxTrackerClient implements MailboxTrackerClient {
	async subscribeActiveMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn> {
		handler([PRODUCTION_MAILBOX_ID]);
		return noop;
	}

	async subscribeAllMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn> {
		handler([PRODUCTION_MAILBOX_ID]);
		return noop;
	}

	async subscribeConnectionState(
		handler: (state: MailboxConnectionState) => void,
		_mailboxId: MailboxId,
	): Promise<UnsubscribeFn> {
		handler({
			status: 'Active',
			consecutive_errors: 0,
			next_poll_in_ms: 5000,
			last_success_at: new Date().toISOString(),
			last_error: null,
		});
		return noop;
	}

	async subscribeSyncState(
		handler: (state: MailboxSyncState) => void,
		_mailboxId: MailboxId,
	): Promise<UnsubscribeFn> {
		handler(syncedToInfinity);
		return noop;
	}
}
