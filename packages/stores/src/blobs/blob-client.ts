import type { Hash } from '../p2panda/types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

/** One row of `get_blob_progress`: the bytes of a blob present in the local
 * store and whether the blob is complete. */
export interface BlobProgress {
	hash: Hash;
	bytes: number;
	complete: boolean;
}

export interface IBlobClient {
	getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]>;
}

export class BlobClient implements IBlobClient {
	getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		return invokeAfterSetup('get_blob_progress', { hashes });
	}
}
