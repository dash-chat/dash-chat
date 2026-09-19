import assert from 'node:assert/strict';
import { beforeEach, describe, it } from 'node:test';

import {
	BLOB_STALL_INTERVAL_MS,
	BlobProgressTracker,
} from '../src/blobs/blob-progress-tracker';

describe('BlobProgressTracker', () => {
	let now: number;
	let tracker: BlobProgressTracker;

	beforeEach(() => {
		now = 1_000_000;
		tracker = new BlobProgressTracker(BLOB_STALL_INTERVAL_MS, () => now);
	});

	it('has no state until the first snapshot', () => {
		assert.equal(tracker.state, undefined);
		assert.deepEqual(tracker.apply({ bytes: 0, complete: false }), {
			bytes: 0,
			complete: false,
			stalled: false,
		});
	});

	it('reports the bytes of the latest snapshot', () => {
		tracker.apply({ bytes: 500, complete: false });
		assert.equal(tracker.apply({ bytes: 800, complete: false }).bytes, 800);
	});

	it('marks a complete snapshot complete and never stalled', () => {
		tracker.apply({ bytes: 0, complete: false });
		now += BLOB_STALL_INTERVAL_MS * 2;
		assert.deepEqual(tracker.apply({ bytes: 800, complete: true }), {
			bytes: 800,
			complete: true,
			stalled: false,
		});
	});

	it('stalls once the bytes have not advanced for the interval', () => {
		tracker.apply({ bytes: 0, complete: false });
		now += BLOB_STALL_INTERVAL_MS - 1;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, false);
		now += 1;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, true);
	});

	it('measures the stall from the first snapshot, not from construction', () => {
		now += BLOB_STALL_INTERVAL_MS * 3;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, false);
	});

	it('restarts the stall clock whenever bytes advance', () => {
		tracker.apply({ bytes: 0, complete: false });
		now += BLOB_STALL_INTERVAL_MS - 1;
		tracker.apply({ bytes: 100, complete: false });
		now += BLOB_STALL_INTERVAL_MS - 1;
		assert.equal(tracker.apply({ bytes: 100, complete: false }).stalled, false);
		now += 1;
		assert.equal(tracker.apply({ bytes: 100, complete: false }).stalled, true);
	});

	it('clears a stall when bytes advance again', () => {
		tracker.apply({ bytes: 0, complete: false });
		now += BLOB_STALL_INTERVAL_MS;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, true);
		assert.equal(tracker.apply({ bytes: 10, complete: false }).stalled, false);
	});

	it('retry clears the stall and restarts the clock', () => {
		tracker.apply({ bytes: 0, complete: false });
		now += BLOB_STALL_INTERVAL_MS;
		tracker.apply({ bytes: 0, complete: false });
		assert.equal(tracker.retry()?.stalled, false);
		now += BLOB_STALL_INTERVAL_MS - 1;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, false);
		now += 1;
		assert.equal(tracker.apply({ bytes: 0, complete: false }).stalled, true);
	});

	it('retry before any snapshot leaves the state unknown', () => {
		assert.equal(tracker.retry(), undefined);
		assert.equal(tracker.state, undefined);
	});
});
