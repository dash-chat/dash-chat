import { listen } from '@tauri-apps/api/event';
import { UnsubscribeFunction } from 'emittery';

import { Hash } from '../p2panda/types';
import { ChatId, SystemEvent, Tombstone, TombstoneReason } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface ITombstoneClient {
	getTombstones(chatId: ChatId): Promise<Record<Hash, TombstoneReason>>;
	onNewTombstones(
		chatId: ChatId,
		handler: (tombstone: Tombstone) => void,
	): UnsubscribeFunction;
}

export class TombstoneClient implements ITombstoneClient {
	getTombstones(chatId: ChatId): Promise<Record<Hash, TombstoneReason>> {
		return invokeAfterSetup('get_tombstones', { chatId });
	}

	onNewTombstones(
		chatId: ChatId,
		handler: (tombstone: Tombstone) => void,
	): UnsubscribeFunction {
		let unsubs: (() => void) | undefined;
		listen('dashchat://system-event', e => {
			const event = e.payload as SystemEvent;
			if (event.type !== 'Tombstones') return;
			if (event.payload.topic !== chatId) return;
			event.payload.hashes.forEach(hash => {
				handler({ hash, reason: event.payload.reason });
			});
		}).then(u => (unsubs = u));

		return () => {
			if (unsubs) unsubs();
		};
	}
}
