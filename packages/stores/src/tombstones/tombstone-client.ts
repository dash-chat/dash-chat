import { UnsubscribeFunction } from 'emittery';

import { Hash } from '../p2panda/types';
import { ChatId, Tombstone, TombstoneReason } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';
import { onSystemEvent } from '../utils/system-events';

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
		return onSystemEvent('Tombstones', event => {
			if (event.payload.topic !== chatId) return;
			event.payload.hashes.forEach(hash => {
				handler({ hash, reason: event.payload.reason });
			});
		});
	}
}
