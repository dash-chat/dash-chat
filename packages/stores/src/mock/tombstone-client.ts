import type { UnsubscribeFunction } from 'emittery';

import type { ITombstoneClient } from '../tombstones/tombstone-client';
import type { TopicId } from '../p2panda/types';
import type { Tombstone } from '../types';

export class MockTombstoneClient implements ITombstoneClient {
	async getTombstones(_topic: TopicId): Promise<Tombstone[]> {
		return [];
	}
	onNewTombstones(): UnsubscribeFunction {
		return () => {};
	}
}
