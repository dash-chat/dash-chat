import { type ReactivePromise, reactive, relay } from 'signalium';

import type { Hash } from '../p2panda/types';
import type { IBlobClient } from './blob-client';
import { BlobProgressTracker, type BlobState } from './blob-progress-tracker';

/** How often the backend is asked about the blobs still downloading. */
export const BLOB_POLL_INTERVAL_MS = 500;

interface Entry {
	tracker: BlobProgressTracker;
	publish: (state: BlobState) => void;
}

/** Download state of the blobs currently on screen. Only blobs with a live
 * `progress` subscription are polled, all of them in one request per tick, and
 * a blob leaves the poll once it is complete. */
export class BlobStore {
	/** The hashes the latest poll asked about; what a test reads to check
	 * that only the blobs on screen are polled. */
	lastPolled: Hash[] = [];
	private entries = new Map<Hash, Entry>();
	private timer: ReturnType<typeof setTimeout> | undefined;
	private polling = false;
	private pollAgainSoon = false;

	constructor(
		public client: IBlobClient,
		private pollMs: number = BLOB_POLL_INTERVAL_MS,
	) {}

	/** Bytes of one blob present locally, whether it is complete, and whether
	 * its download has stalled. The total size comes from the attachment's own
	 * metadata. Pending until the first snapshot resolves, so a caller can tell
	 * "unknown" from "0 bytes" and never show a ring on a blob it has not asked
	 * about. */
	progress = reactive(
		(hash: Hash): ReactivePromise<BlobState> =>
			relay(state => {
				const entry: Entry = {
					tracker: new BlobProgressTracker(),
					publish: next => {
						if (!sameState(state.value, next)) state.value = next;
					},
				};
				this.entries.set(hash, entry);
				this.schedule(0);
				return () => {
					if (this.entries.get(hash) === entry) this.entries.delete(hash);
				};
			}),
	);

	/** Clear a blob's stalled flag after the user re-attempted its download. */
	retry(hash: Hash): void {
		const entry = this.entries.get(hash);
		const next = entry?.tracker.retry();
		if (entry !== undefined && next !== undefined) entry.publish(next);
	}

	private incompleteHashes(): Hash[] {
		const hashes: Hash[] = [];
		this.entries.forEach((entry, hash) => {
			if (entry.tracker.state?.complete !== true) hashes.push(hash);
		});
		return hashes;
	}

	private schedule(ms: number): void {
		if (this.polling) {
			if (ms === 0) this.pollAgainSoon = true;
			return;
		}
		if (this.timer !== undefined) {
			if (ms > 0) return;
			clearTimeout(this.timer);
		}
		this.timer = setTimeout(() => {
			this.timer = undefined;
			void this.poll();
		}, ms);
	}

	private async poll(): Promise<void> {
		const hashes = this.incompleteHashes();
		if (hashes.length === 0) return;
		this.polling = true;
		this.lastPolled = hashes;
		try {
			const snapshots = await this.client.getBlobProgress(hashes);
			for (const snapshot of snapshots) {
				const entry = this.entries.get(snapshot.hash);
				if (entry !== undefined) entry.publish(entry.tracker.apply(snapshot));
			}
		} catch (e) {
			console.error('blob progress poll failed', e);
		} finally {
			this.polling = false;
		}
		const soon = this.pollAgainSoon;
		this.pollAgainSoon = false;
		this.schedule(soon ? 0 : this.pollMs);
	}
}

function sameState(a: BlobState | undefined, b: BlobState): boolean {
	return (
		a !== undefined &&
		a.bytes === b.bytes &&
		a.complete === b.complete &&
		a.stalled === b.stalled
	);
}
