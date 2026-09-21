import type { BlobProgress, IBlobClient } from '../blobs/blob-client';
import type { Hash } from '../p2panda/types';

/** Mock mode serves every blob from memory, so nothing is ever downloading. */
export class MockBlobClient implements IBlobClient {
	async getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		return hashes.map(hash => ({ hash, bytes: 0, complete: true }));
	}

	async fetchBlobNow(): Promise<void> {}
}
