import { listen } from '@tauri-apps/api/event';
import { UnsubscribeFunction } from 'emittery';

import { Hash } from '../p2panda/types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';
import type { BlobProgress } from './blob-progress-tracker';

export interface IBlobClient {
	getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]>;
	fetchBlobNow(hash: Hash): Promise<void>;
	onBlobProgress(
		hash: Hash,
		handler: (progress: BlobProgress) => void,
	): UnsubscribeFunction;
}

export class BlobClient implements IBlobClient {
	getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		return invokeAfterSetup('get_blob_progress', { hashes });
	}

	fetchBlobNow(hash: Hash): Promise<void> {
		return invokeAfterSetup('fetch_blob_now', { hash });
	}

	onBlobProgress(
		hash: Hash,
		handler: (progress: BlobProgress) => void,
	): UnsubscribeFunction {
		let unsub: (() => void) | undefined;
		listen('blob://progress', e => {
			const progress = e.payload as BlobProgress;
			if (progress.hash !== hash) return;
			handler(progress);
		}).then(u => (unsub = u));
		return () => {
			if (unsub) unsub();
		};
	}
}
