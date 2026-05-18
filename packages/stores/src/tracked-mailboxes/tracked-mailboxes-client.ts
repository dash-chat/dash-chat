import { Channel, invoke } from '@tauri-apps/api/core';

import type {
	MailboxConnectionState,
	MailboxId,
	SyncStateEntry,
} from './types';

export interface TrackedMailboxesClient {
	subscribeTrackedMailboxIds(handler: (ids: MailboxId[]) => void): UnsubscribeFn;
	subscribeConnectionState(
		mailboxId: MailboxId,
		handler: (state: MailboxConnectionState) => void,
	): UnsubscribeFn;
	subscribeSyncState(
		mailboxId: MailboxId,
		handler: (entries: SyncStateEntry[]) => void,
	): UnsubscribeFn;
}

export type UnsubscribeFn = () => void;

interface TauriChannelInternals {
	id: number;
}

interface TauriInternals {
	unregisterCallback?(id: number): void;
}

declare global {
	interface Window {
		__TAURI_INTERNALS__?: TauriInternals;
	}
}

function unregister(channelId: number): void {
	if (typeof window === 'undefined') return;
	window.__TAURI_INTERNALS__?.unregisterCallback?.(channelId);
}

export class TauriTrackedMailboxesClient implements TrackedMailboxesClient {
	subscribeTrackedMailboxIds(handler: (ids: MailboxId[]) => void): UnsubscribeFn {
		const channel = new Channel<MailboxId[]>();
		channel.onmessage = handler;
		invoke('mailbox_subscribe_ids', { onEvent: channel }).catch(err =>
			console.error('mailbox_subscribe_ids failed', err),
		);
		return () => unregister((channel as unknown as TauriChannelInternals).id);
	}

	subscribeConnectionState(
		mailboxId: MailboxId,
		handler: (state: MailboxConnectionState) => void,
	): UnsubscribeFn {
		const channel = new Channel<MailboxConnectionState>();
		channel.onmessage = handler;
		invoke('mailbox_subscribe_connection_state', {
			mailboxId,
			onEvent: channel,
		}).catch(err =>
			console.error(
				`mailbox_subscribe_connection_state(${mailboxId}) failed`,
				err,
			),
		);
		return () => unregister((channel as unknown as TauriChannelInternals).id);
	}

	subscribeSyncState(
		mailboxId: MailboxId,
		handler: (entries: SyncStateEntry[]) => void,
	): UnsubscribeFn {
		const channel = new Channel<SyncStateEntry[]>();
		channel.onmessage = handler;
		invoke('mailbox_subscribe_sync_state', {
			mailboxId,
			onEvent: channel,
		}).catch(err =>
			console.error(`mailbox_subscribe_sync_state(${mailboxId}) failed`, err),
		);
		return () => unregister((channel as unknown as TauriChannelInternals).id);
	}
}
