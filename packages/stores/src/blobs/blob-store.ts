import { type ReactivePromise, reactive, relay } from 'signalium';

import type { Hash } from '../p2panda/types';
import { pollingRequired } from '../utils/polling-required';
import type { IBlobClient } from './blob-client';
import { BlobProgressTracker, type BlobState } from './blob-progress-tracker';

const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED = pollingRequired();

export class BlobStore {
	#trackers = new Map<Hash, BlobProgressTracker>();

	constructor(public client: IBlobClient) {}

	/** Download state of one blob: bytes present locally, whether it is
	 * complete, and whether the download has stalled. The total size comes
	 * from the attachment's own metadata, not from here. */
	progress = reactive(
		(hash: Hash): ReactivePromise<BlobState> =>
			relay(state => {
				const tracker = new BlobProgressTracker(s => {
					state.value = s;
				});
				this.#trackers.set(hash, tracker);
				state.value = tracker.state;

				const fetchProgress = async () => {
					const [progress] = await this.client.getBlobProgress([hash]);
					if (progress) tracker.apply(progress);
				};

				fetchProgress();
				const interval = POLLING_ENABLED
					? setInterval(fetchProgress, POLL_INTERVAL_MS)
					: undefined;
				const unsub = this.client.onBlobProgress(hash, p => tracker.apply(p));

				return () => {
					clearInterval(interval);
					unsub();
					tracker.dispose();
					if (this.#trackers.get(hash) === tracker) this.#trackers.delete(hash);
				};
			}),
	);

	/** Re-attempt a stalled download and clear its stalled flag. */
	async retry(hash: Hash): Promise<void> {
		this.#trackers.get(hash)?.retry();
		await this.client.fetchBlobNow(hash);
	}
}
