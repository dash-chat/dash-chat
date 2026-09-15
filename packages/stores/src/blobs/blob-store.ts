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
				let interval: ReturnType<typeof setInterval> | undefined;
				// Signalium's build transform evaluates a closure's captured
				// bindings where the closure is defined, so nothing declared
				// before `tracker` may name it directly.
				let onStall: (() => void) | undefined;

				const stopPolling = () => {
					clearInterval(interval);
					interval = undefined;
				};

				const tracker = new BlobProgressTracker(s => {
					state.value = s;
					if (s.complete) stopPolling();
					// A completion that landed while no listener was attached is never
					// replayed, so a stall re-reads the snapshot to self-heal.
					else if (s.stalled) onStall?.();
				});
				this.#trackers.set(hash, tracker);
				state.value = tracker.state;

				const fetchProgress = () => {
					this.client
						.getBlobProgress([hash])
						.then(([progress]) => {
							if (progress) tracker.apply(progress);
						})
						.catch(e => console.error('blob progress snapshot failed', e));
				};
				onStall = fetchProgress;

				fetchProgress();
				if (POLLING_ENABLED)
					interval = setInterval(fetchProgress, POLL_INTERVAL_MS);
				const unsub = this.client.onBlobProgress(hash, p => tracker.apply(p));

				return () => {
					stopPolling();
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
