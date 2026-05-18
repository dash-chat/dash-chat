import type { DeviceId, TopicId } from '../p2panda/types';

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

/// Synced-up-to sequence number per (topic, author), shaped as `topic → author → seq`.
export type MailboxSyncState = Record<TopicId, Record<DeviceId, number>>;
