import { Channel, invoke } from '@tauri-apps/api/core';

import { unregisterChannel } from '../utils/tauri-channel';
import type { MailboxId, MailboxSyncState, MailboxTracker } from './types';

export interface MailboxTrackerClient {
	subscribeActiveMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn>;
	subscribeAllMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn>;
	subscribeTracker(
		mailboxId: MailboxId,
		handler: (state: MailboxTracker) => void,
	): Promise<UnsubscribeFn>;
	subscribeSyncState(
		mailboxId: MailboxId,
		handler: (state: MailboxSyncState) => void,
	): Promise<UnsubscribeFn>;
}

export type UnsubscribeFn = () => void;

export class TauriMailboxTrackerClient implements MailboxTrackerClient {
	async subscribeActiveMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxId[]>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_active_ids', { onEvent: channel });
		return () => unregisterChannel(channel);
	}

	async subscribeAllMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxId[]>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_all_ids', { onEvent: channel });
		return () => unregisterChannel(channel);
	}

	async subscribeTracker(
		mailboxId: MailboxId,
		handler: (state: MailboxTracker) => void,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxTracker>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_tracker', {
			mailboxId,
			onEvent: channel,
		});
		return () => unregisterChannel(channel);
	}

	async subscribeSyncState(
		mailboxId: MailboxId,
		handler: (state: MailboxSyncState) => void,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxSyncState>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_sync_state', {
			mailboxId,
			onEvent: channel,
		});
		return () => unregisterChannel(channel);
	}
}
