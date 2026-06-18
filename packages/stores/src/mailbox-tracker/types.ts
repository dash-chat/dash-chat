import type { DeviceId, LogId } from '../p2panda/types';

export type MailboxId = string;

/// Id of the production cloud mailbox.
/// Kept in sync with `PRODUCTION_MAILBOX_ID` in `src-tauri/src/mailbox.rs`.
export const PRODUCTION_MAILBOX_ID: MailboxId = 'dashchat-mailbox';

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

/// Synced-up-to sequence number per (log, author), shaped as `log → author → seq`.
export type MailboxSyncState = Record<LogId, Record<DeviceId, number>>;
