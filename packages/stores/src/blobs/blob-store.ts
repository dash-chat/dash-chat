import { type ReactivePromise, reactive, relay } from 'signalium';

import type { Hash } from '../p2panda/types';
import { pollingRequired } from '../utils/polling-required';
import type { IBlobClient } from './blob-client';
import { BlobProgressTracker, type BlobState } from './blob-progress-tracker';

const POLL_INTERVAL_MS = 1_000;
const POLLING_ENABLED = pollingRequired();
/** While stalled, re-read the snapshot on a timer: a completion event that
 * landed while no listener was attached is never replayed. */
const STALLED_POLL_INTERVAL_MS = 5_000;

export class BlobStore {
	#trackers = new Map<Hash, BlobProgressTracker>();

	constructor(public client: IBlobClient) {}

	/** Download state of one blob: bytes present locally, whether it is
	 * complete, and whether the download has stalled. The total size comes
	 * from the attachment's own metadata, not from here. Pending until the
	 * first snapshot arrives, so a caller can tell "unknown" from "0 bytes". */
	progress = reactive(
		(hash: Hash): ReactivePromise<BlobState> =>
			relay(state => {
				let interval: ReturnType<typeof setInterval> | undefined;
				let kicked = false;
				// Signalium's build transform evaluates a closure's captured
				// bindings where the closure is defined, so nothing declared
				// before `tracker` may name it directly.
				let setStalledPolling: ((stalled: boolean) => void) | undefined;

				const stopPolling = () => {
					clearInterval(interval);
					interval = undefined;
				};

				const tracker = new BlobProgressTracker(s => {
					state.value = s;
					if (s.complete) stopPolling();
					else if (!POLLING_ENABLED) setStalledPolling?.(s.stalled);
				});
				this.#trackers.set(hash, tracker);

				const fetchProgress = () => {
					this.client
						.getBlobProgress([hash])
						.then(([progress]) => {
							if (!progress) return;
							tracker.apply(progress);
							// Seeing an incomplete blob is what starts its download, as
							// mounting the <img> did before the ring replaced it.
							if (!progress.complete && !kicked) {
								kicked = true;
								void this.client.fetchBlobNow(hash);
							}
						})
						.catch(e => console.error('blob progress snapshot failed', e));
				};
				const startPolling = (ms: number) => {
					if (interval === undefined) interval = setInterval(fetchProgress, ms);
				};
				setStalledPolling = stalled =>
					stalled ? startPolling(STALLED_POLL_INTERVAL_MS) : stopPolling();

				fetchProgress();
				if (POLLING_ENABLED) startPolling(POLL_INTERVAL_MS);
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
