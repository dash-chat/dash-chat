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

type ProgressHandler = (progress: BlobProgress) => void;

export class BlobClient implements IBlobClient {
	#handlers = new Map<Hash, Set<ProgressHandler>>();
	#listening = false;

	getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		return invokeAfterSetup('get_blob_progress', { hashes });
	}

	fetchBlobNow(hash: Hash): Promise<void> {
		return invokeAfterSetup('fetch_blob_now', { hash });
	}

	/** One Tauri listener for every blob, demultiplexed by hash, rather than
	 * one per attachment on screen. */
	onBlobProgress(hash: Hash, handler: ProgressHandler): UnsubscribeFunction {
		this.#ensureListening();
		let handlers = this.#handlers.get(hash);
		if (!handlers) {
			handlers = new Set();
			this.#handlers.set(hash, handlers);
		}
		handlers.add(handler);
		return () => {
			handlers.delete(handler);
			if (handlers.size === 0) this.#handlers.delete(hash);
		};
	}

	#ensureListening(): void {
		if (this.#listening) return;
		this.#listening = true;
		void listen<BlobProgress>('blob://progress', e => {
			const handlers = this.#handlers.get(e.payload.hash);
			if (!handlers) return;
			for (const handler of handlers) handler(e.payload);
		});
	}
}
