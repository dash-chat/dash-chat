import type { DeviceId, LogId, TopicId } from '../p2panda/types';

export type MailboxId = string;

export type SyncStatus = 'Active' | 'Degraded' | 'Stopped';

export interface MailboxConnectionState {
	status: SyncStatus;
	consecutive_errors: number;
	/// Milliseconds from now until the next scheduled poll; negative if overdue.
	next_poll_in_ms: number;
	last_success_at: string | null;
	last_error: LastError | null;
}

export interface LastError {
	at: string;
	message: string;
}

/// The key a mailbox files one of a device's logs under within a topic.
export type MailboxLogKey = string;

export function mailboxLogKey(author: DeviceId, logId: LogId): MailboxLogKey {
	return `${author}.${logId}`;
}

/// Synced-up-to sequence number per log, shaped as `topic → log key → seq`.
export type MailboxSyncState = Record<TopicId, Record<MailboxLogKey, number>>;
