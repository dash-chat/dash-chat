import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, it } from 'node:test';
import { watcher } from 'signalium';

import type { BlobProgress, IBlobClient } from '../src/blobs/blob-client';
import type { BlobState } from '../src/blobs/blob-progress-tracker';
import { BlobStore } from '../src/blobs/blob-store';
import type { Hash } from '../src/p2panda/types';

const POLL_MS = 5;

/** Answers every poll from `snapshots`, recording which hashes were asked
 * about; a hash with no snapshot is left out of the answer. */
class FakeBlobClient implements IBlobClient {
	polls: Hash[][] = [];
	fetches: Hash[] = [];
	snapshots = new Map<Hash, BlobProgress>();
	failNext = false;

	async getBlobProgress(hashes: Hash[]): Promise<BlobProgress[]> {
		this.polls.push([...hashes]);
		if (this.failNext) {
			this.failNext = false;
			throw new Error('node not ready');
		}
		return hashes.flatMap(hash => this.snapshots.get(hash) ?? []);
	}

	async fetchBlobNow(hash: Hash): Promise<void> {
		this.fetches.push(hash);
	}

	set(hash: Hash, bytes: number, complete = false): void {
		this.snapshots.set(hash, { hash, bytes, complete });
	}
}

/** Subscribe to a blob's progress the way `useReactiveValue` does, collecting
 * every state it publishes. Returns the unsubscribe. */
function subscribe(store: BlobStore, hash: Hash, seen: BlobState[]) {
	const w = watcher(() => {
		const rp = store.progress(hash);
		(rp as unknown as { _version: { value: unknown } })._version.value;
		return rp.isReady ? rp.value : undefined;
	});
	const read = () => {
		if (w.value !== undefined) seen.push(w.value);
	};
	const unsub = w.addListener(read);
	read();
	return unsub;
}

function sleep(ms: number): Promise<void> {
	return new Promise(resolve => setTimeout(resolve, ms));
}

async function until(cond: () => boolean, what: string): Promise<void> {
	const deadline = Date.now() + 2_000;
	while (!cond()) {
		assert.ok(Date.now() < deadline, `timed out waiting for ${what}`);
		await sleep(POLL_MS);
	}
}

describe('BlobStore.progress', () => {
	let client: FakeBlobClient;
	let store: BlobStore;
	const unsubs: (() => void)[] = [];

	beforeEach(() => {
		client = new FakeBlobClient();
		store = new BlobStore(client, {
			pollMs: POLL_MS,
			stalledPollMs: POLL_MS * 20,
			stallMs: POLL_MS * 4,
		});
	});

	afterEach(() => {
		unsubs.splice(0).forEach(unsub => unsub());
	});

	it('is unknown until the first snapshot, then reports it', async () => {
		client.set('h1', 0);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		assert.deepEqual(seen, []);
		await until(() => seen.length > 0, 'the first snapshot');
		assert.deepEqual(seen[0], { bytes: 0, complete: false, stalled: false });
	});

	it('polls every subscribed incomplete blob in one request', async () => {
		client.set('h1', 0);
		client.set('h2', 0);
		unsubs.push(subscribe(store, 'h1', []), subscribe(store, 'h2', []));
		await until(() => client.polls.length >= 2, 'two polls');
		assert.deepEqual(client.polls[1], ['h1', 'h2']);
		assert.deepEqual(store.lastPolled, ['h1', 'h2']);
	});

	it('stops polling a blob once it is complete', async () => {
		client.set('h1', 0);
		client.set('h2', 0);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen), subscribe(store, 'h2', []));
		await until(() => seen.length > 0, 'the first snapshot');
		client.set('h1', 800, true);
		await until(() => seen.at(-1)?.complete === true, 'completion');
		const before = client.polls.length;
		await until(() => client.polls.length > before + 1, 'later polls');
		assert.deepEqual(client.polls.at(-1), ['h2']);
	});

	it('stops polling altogether when nothing is incomplete', async () => {
		client.set('h1', 800, true);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		await until(() => seen.length > 0, 'the first snapshot');
		const before = client.polls.length;
		await sleep(POLL_MS * 5);
		assert.equal(client.polls.length, before);
	});

	it('stops polling a blob nobody subscribes to any more', async () => {
		client.set('h1', 0);
		const unsub = subscribe(store, 'h1', []);
		await until(() => client.polls.length > 0, 'the first poll');
		unsub();
		await sleep(POLL_MS * 3);
		const before = client.polls.length;
		await sleep(POLL_MS * 5);
		assert.equal(client.polls.length, before);
	});

	it('publishes only when the state changes', async () => {
		client.set('h1', 0);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		await until(() => client.polls.length >= 4, 'several polls');
		assert.equal(seen.length, 1);
		client.set('h1', 10);
		await until(() => seen.length === 2, 'the advance');
		assert.equal(seen[1].bytes, 10);
	});

	it('keeps the state unknown and polls again when a poll fails', async () => {
		client.set('h1', 0);
		client.failNext = true;
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		await until(() => client.polls.length >= 2, 'the retry poll');
		await until(() => seen.length > 0, 'the snapshot after the failure');
		assert.deepEqual(seen, [{ bytes: 0, complete: false, stalled: false }]);
	});

	it('drops a blob the poll returned no row for', async () => {
		client.set('h2', 0);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen), subscribe(store, 'h2', []));
		await until(() => client.polls.length >= 3, 'later polls');
		assert.deepEqual(client.polls[0], ['h1', 'h2']);
		assert.deepEqual(client.polls.at(-1), ['h2']);
		assert.deepEqual(seen, []);
	});

	it('reports the last known state at once when subscribed to again', async () => {
		client.set('h1', 10);
		const first: BlobState[] = [];
		const unsub = subscribe(store, 'h1', first);
		await until(() => first.length > 0, 'the first snapshot');
		unsub();
		await sleep(POLL_MS * 3);
		client.set('h1', 20);
		const again: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', again));
		assert.deepEqual(again, [{ bytes: 10, complete: false, stalled: false }]);
		await until(() => again.at(-1)?.bytes === 20, 'the next snapshot');
	});

	it('keeps the stall clock running while nothing shows the blob', async () => {
		client.set('h1', 0);
		const unsub = subscribe(store, 'h1', []);
		await until(() => client.polls.length > 0, 'the first poll');
		unsub();
		await sleep(POLL_MS * 6);
		const before = client.polls.length;
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		await until(() => seen.at(-1)?.stalled === true, 'the stall');
		assert.ok(
			client.polls.length - before <= 1,
			`took ${client.polls.length - before} polls to notice the stall`,
		);
	});

	it('reports a complete blob as complete at once when subscribed to again', async () => {
		client.set('h1', 800, true);
		const first: BlobState[] = [];
		const unsub = subscribe(store, 'h1', first);
		await until(() => first.length > 0, 'the first snapshot');
		unsub();
		await sleep(POLL_MS * 3);
		const again: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', again));
		assert.deepEqual(again, [{ bytes: 800, complete: true, stalled: false }]);
	});

	it('flags a stall, then polls slowly until a retry', async () => {
		client.set('h1', 0);
		const seen: BlobState[] = [];
		unsubs.push(subscribe(store, 'h1', seen));
		await until(() => seen.at(-1)?.stalled === true, 'the stall');
		const before = client.polls.length;
		await sleep(POLL_MS * 8);
		assert.ok(
			client.polls.length <= before + 1,
			`polled ${client.polls.length - before} times while stalled`,
		);
		store.retry('h1');
		assert.deepEqual(client.fetches, ['h1']);
		await until(() => seen.at(-1)?.stalled === false, 'the cleared stall');
		await until(() => client.polls.length > before + 1, 'the poll after retry');
	});
});
