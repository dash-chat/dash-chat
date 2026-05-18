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

export interface SyncStateEntry {
	topic_id: TopicId;
	author: DeviceId;
	seq_num: number;
}

/// Map keyed by `${topic_id}:${author}` for cheap reactive lookups.
export type MailboxSyncState = Map<string, number>;

export function syncKey(topicId: TopicId, author: DeviceId): string {
	return `${topicId}:${author}`;
}
