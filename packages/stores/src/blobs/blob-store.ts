import { type ReactivePromise, reactive, relay } from 'signalium';

import type { Hash } from '../p2panda/types';
import type { IBlobClient } from './blob-client';
import {
	BLOB_STALL_INTERVAL_MS,
	BlobProgressTracker,
	type BlobState,
} from './blob-progress-tracker';

/** How often the backend is asked about the blobs still downloading. */
export const BLOB_POLL_INTERVAL_MS = 500;

/** How often it is asked once every download on screen has stalled: nothing
 * is moving, so a slower look costs nothing in responsiveness. */
export const BLOB_STALLED_POLL_INTERVAL_MS = 5_000;

export interface BlobStoreTiming {
	pollMs: number;
	stalledPollMs: number;
	stallMs: number;
}

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
		private timing: BlobStoreTiming = {
			pollMs: BLOB_POLL_INTERVAL_MS,
			stalledPollMs: BLOB_STALLED_POLL_INTERVAL_MS,
			stallMs: BLOB_STALL_INTERVAL_MS,
		},
	) {}

	/** Bytes of one blob present locally, whether it is complete, and whether
	 * its download has stalled. The total size comes from the attachment's own
	 * metadata. Pending until the first snapshot resolves, so a caller can tell
	 * "unknown" from "0 bytes" and never show a ring on a blob it has not asked
	 * about; a blob the node returns no row for stays pending and is not asked
	 * about again. */
	progress = reactive(
		(hash: Hash): ReactivePromise<BlobState> =>
			relay(state => {
				const entry: Entry = {
					tracker: new BlobProgressTracker(this.timing.stallMs),
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

	/** The user tapped a blob that is still downloading: ask the node to fetch
	 * it now, clear its stalled flag, and look again at once. */
	retry(hash: Hash): void {
		this.client
			.fetchBlobNow(hash)
			.catch(e => console.error('blob fetch request failed', e));
		const entry = this.entries.get(hash);
		const next = entry?.tracker.retry();
		if (entry !== undefined && next !== undefined) entry.publish(next);
		this.schedule(0);
	}

	private incompleteEntries(): Map<Hash, Entry> {
		const incomplete = new Map<Hash, Entry>();
		this.entries.forEach((entry, hash) => {
			if (entry.tracker.state?.complete !== true) incomplete.set(hash, entry);
		});
		return incomplete;
	}

	private everyDownloadStalled(): boolean {
		let stalled = true;
		this.incompleteEntries().forEach(entry => {
			if (entry.tracker.state?.stalled !== true) stalled = false;
		});
		return stalled;
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
		const polled = this.incompleteEntries();
		if (polled.size === 0) return;
		this.polling = true;
		this.lastPolled = Array.from(polled.keys());
		try {
			const snapshots = await this.client.getBlobProgress(this.lastPolled);
			for (const snapshot of snapshots) {
				const entry = this.entries.get(snapshot.hash);
				if (entry !== undefined) entry.publish(entry.tracker.apply(snapshot));
				polled.delete(snapshot.hash);
			}
			polled.forEach((entry, hash) => {
				if (this.entries.get(hash) === entry) this.entries.delete(hash);
			});
		} catch (e) {
			console.error('blob progress poll failed', e);
		} finally {
			this.polling = false;
		}
		const soon = this.pollAgainSoon;
		this.pollAgainSoon = false;
		if (soon) this.schedule(0);
		else if (this.everyDownloadStalled())
			this.schedule(this.timing.stalledPollMs);
		else this.schedule(this.timing.pollMs);
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
