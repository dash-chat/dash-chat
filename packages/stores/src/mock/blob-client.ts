import { UnsubscribeFunction } from 'emittery';

import type { IBlobClient } from '../blobs/blob-client';
import type { BlobProgress } from '../blobs/blob-progress-tracker';
import { Hash } from '../p2panda/types';

export class MockBlobClient implements IBlobClient {
	async getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		return hashes.map(hash => ({ hash, bytes: 0, complete: true }));
	}

	async fetchBlobNow(): Promise<void> {}

	onBlobProgress(): UnsubscribeFunction {
		return () => {};
	}
}
