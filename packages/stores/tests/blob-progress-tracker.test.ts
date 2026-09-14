import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, it, mock } from 'node:test';

import {
	BLOB_STALL_INTERVAL_MS,
	BlobProgressTracker,
	type BlobState,
} from '../src/blobs/blob-progress-tracker';

describe('BlobProgressTracker', () => {
	let states: BlobState[];
	let tracker: BlobProgressTracker;

	beforeEach(() => {
		mock.timers.enable({ apis: ['setTimeout'] });
		states = [];
		tracker = new BlobProgressTracker(s => states.push(s));
	});

	afterEach(() => {
		tracker.dispose();
		mock.timers.reset();
	});

	it('starts at zero, incomplete, not stalled', () => {
		assert.deepEqual(tracker.state, {
			bytes: 0,
			complete: false,
			stalled: false,
		});
	});

	it('keeps the highest byte count seen', () => {
		tracker.apply({ bytes: 500, complete: false });
		tracker.apply({ bytes: 200, complete: false });
		assert.equal(tracker.state.bytes, 500);
	});

	it('marks complete and never stalls afterwards', () => {
		tracker.apply({ bytes: 800, complete: true });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS * 2);
		assert.deepEqual(tracker.state, {
			bytes: 800,
			complete: true,
			stalled: false,
		});
	});

	it('stalls after the interval with no progress', () => {
		tracker.apply({ bytes: 0, complete: false });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS - 1);
		assert.equal(tracker.state.stalled, false);
		mock.timers.tick(1);
		assert.equal(tracker.state.stalled, true);
	});

	it('restarts the stall timer whenever bytes advance', () => {
		tracker.apply({ bytes: 0, complete: false });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS - 1);
		tracker.apply({ bytes: 100, complete: false });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS - 1);
		assert.equal(tracker.state.stalled, false);
		mock.timers.tick(1);
		assert.equal(tracker.state.stalled, true);
	});

	it('progress after a stall clears it', () => {
		tracker.apply({ bytes: 0, complete: false });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS);
		assert.equal(tracker.state.stalled, true);
		tracker.apply({ bytes: 10, complete: false });
		assert.equal(tracker.state.stalled, false);
	});

	it('retry clears the stall and restarts the timer', () => {
		tracker.apply({ bytes: 0, complete: false });
		mock.timers.tick(BLOB_STALL_INTERVAL_MS);
		tracker.retry();
		assert.equal(tracker.state.stalled, false);
		mock.timers.tick(BLOB_STALL_INTERVAL_MS);
		assert.equal(tracker.state.stalled, true);
	});

	it('notifies only on change', () => {
		tracker.apply({ bytes: 0, complete: false });
		tracker.apply({ bytes: 0, complete: false });
		assert.equal(states.length, 0);
		tracker.apply({ bytes: 1, complete: false });
		assert.equal(states.length, 1);
	});
});
