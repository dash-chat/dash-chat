import { Channel, invoke } from '@tauri-apps/api/core';

import { UnsubscribeFn, unregisterChannel } from '../utils/tauri-channel';
import type {
	MailboxConnectionState,
	MailboxId,
	MailboxSyncState,
} from './types';

export interface MailboxTrackerClient {
	subscribeActiveMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn>;
	subscribeAllMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn>;
	subscribeConnectionState(
		handler: (state: MailboxConnectionState) => void,
		mailboxId: MailboxId,
	): Promise<UnsubscribeFn>;
	subscribeSyncState(
		handler: (state: MailboxSyncState) => void,
		mailboxId: MailboxId,
	): Promise<UnsubscribeFn>;
}

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

	async subscribeConnectionState(
		handler: (state: MailboxConnectionState) => void,
		mailboxId: MailboxId,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxConnectionState>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_connection_state', {
			mailboxId,
			onEvent: channel,
		});
		return () => unregisterChannel(channel);
	}

	async subscribeSyncState(
		handler: (state: MailboxSyncState) => void,
		mailboxId: MailboxId,
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
