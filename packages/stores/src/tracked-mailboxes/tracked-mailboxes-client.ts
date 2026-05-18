import { Channel, invoke } from '@tauri-apps/api/core';

import { unregisterChannel } from '../utils/tauri-channel';

import type {
	MailboxConnectionState,
	MailboxId,
	MailboxSyncState,
} from './types';

export interface TrackedMailboxesClient {
	subscribeTrackedMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn>;
	subscribeConnectionState(
		mailboxId: MailboxId,
		handler: (state: MailboxConnectionState) => void,
	): Promise<UnsubscribeFn>;
	subscribeSyncState(
		mailboxId: MailboxId,
		handler: (state: MailboxSyncState) => void,
	): Promise<UnsubscribeFn>;
}

export type UnsubscribeFn = () => void;

export class TauriTrackedMailboxesClient implements TrackedMailboxesClient {
	async subscribeTrackedMailboxIds(
		handler: (ids: MailboxId[]) => void,
	): Promise<UnsubscribeFn> {
		const channel = new Channel<MailboxId[]>();
		channel.onmessage = handler;
		await invoke('mailbox_subscribe_ids', { onEvent: channel });
		return () => unregisterChannel(channel);
	}

	async subscribeConnectionState(
		mailboxId: MailboxId,
		handler: (state: MailboxConnectionState) => void,
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
