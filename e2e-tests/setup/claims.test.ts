import assert from 'node:assert/strict';
import { type ChildProcess, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdirSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';

import {
	claimAllWhenFreeSync,
	describeHeld,
	isFree,
	release,
} from './claims.ts';

/** Ids no other test or run uses, cleaned up after each test. */
let counter = 0;
function fresh(): string {
	counter += 1;
	return `test-${process.pid}-${counter}`;
}

/** Hold `id` from another live process, until it is killed. */
function heldByOther(id: string): ChildProcess {
	const other = spawn('sleep', ['60'], { stdio: 'ignore' });
	mkdirSync('/tmp/dash-chat-e2e/claims', { recursive: true });
	writeFileSync(`/tmp/dash-chat-e2e/claims/${id}`, String(other.pid));
	return other;
}

test('a claim is exclusive to a live process and stale once it is gone', async () => {
	const id = fresh();
	const other = heldByOther(id);
	try {
		assert.equal(isFree(id), false);
		assert.match(describeHeld([id]), /driven by another e2e run: test-/);
	} finally {
		other.kill('SIGKILL');
	}
	await once(other, 'exit');
	assert.equal(isFree(id), true);
	assert.deepEqual(claimAllWhenFreeSync([{ candidates: [id], needed: 1 }]), [
		[id],
	]);
	assert.equal(isFree(id), true);
	release(id);
});

test('a run takes the free candidates first and all of its groups at once', async () => {
	const busy = fresh();
	const a = fresh();
	const b = fresh();
	const other = heldByOther(busy);
	try {
		const taken = claimAllWhenFreeSync([
			{ candidates: [busy, a, b], needed: 2 },
		]);
		assert.deepEqual(taken, [[a, b]]);
	} finally {
		other.kill('SIGKILL');
		for (const id of [a, b]) release(id);
	}
	await once(other, 'exit');
});

test('a group short of candidates throws instead of waiting forever', () => {
	const id = fresh();
	assert.throws(
		() => claimAllWhenFreeSync([{ candidates: [id], needed: 2 }]),
		/cannot claim 2 of connected: test-/,
	);
	assert.equal(isFree(id), true);
});
