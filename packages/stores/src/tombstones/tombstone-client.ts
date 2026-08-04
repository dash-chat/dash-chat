import { listen } from '@tauri-apps/api/event';
import { UnsubscribeFunction } from 'emittery';

import { TopicId as chatId } from '../p2panda/types';
import { ChatId, SystemEvent, Tombstone } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface ITombstoneClient {
	getTombstones(chatId: chatId): Promise<Tombstone[]>;
	onNewTombstones(
		chatId: chatId,
		handler: (tombstone: Tombstone) => void,
	): UnsubscribeFunction;
}

export class TombstoneClient implements ITombstoneClient {
	getTombstones(chatId: ChatId): Promise<Tombstone[]> {
		return invokeAfterSetup('get_tombstones', { chatId });
	}

	onNewTombstones(
		chatId: chatId,
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
