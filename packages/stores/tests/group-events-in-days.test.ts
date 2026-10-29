import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import type { DeviceId, Hash } from '../src/p2panda/types';
import {
	EventWithProvenance,
	MESSAGE_SET_TIMEFRAME_INTERVAL_MS,
	groupEventsInDays,
} from '../src/utils/group-events-in-days';

const ME: DeviceId = 'me';
const PEER: DeviceId = 'peer';
const AGENT_SETS = [[ME], [PEER]];

const BASE_TS = new Date(2026, 0, 5, 12, 0, 0).valueOf();

interface TestEvent {
	hash: Hash;
}

function events(
	specs: Array<{
		hash: Hash;
		author?: DeviceId;
		offsetMs?: number;
		lamport?: number;
		type?: string;
	}>,
): Record<Hash, EventWithProvenance<TestEvent>> {
	const out: Record<Hash, EventWithProvenance<TestEvent>> = {};
	for (let i = 0; i < specs.length; i++) {
		const spec = specs[i];
		out[spec.hash] = {
			event: { hash: spec.hash },
			author: spec.author ?? ME,
			timestamp: BASE_TS + (spec.offsetMs ?? i * 1000),
			lamport: spec.lamport ?? 0,
			type: spec.type ?? 'Message',
		};
	}
	return out;
}

/** The grouping result as arrays of hashes, flattened across days. */
function groupHashes(
	input: Record<Hash, EventWithProvenance<TestEvent>>,
): Hash[][] {
	return groupEventsInDays(input, AGENT_SETS).flatMap(day =>
		day.eventsGroups.map(group => group.map(([hash]) => hash)),
	);
}

describe('groupEventsInDays', () => {
	it('keeps consecutive events of one author in one group', () => {
		const input = events([{ hash: 'a' }, { hash: 'b' }, { hash: 'c' }]);
		assert.deepEqual(groupHashes(input), [['a', 'b', 'c']]);
	});

	it('splits by author', () => {
		const input = events([
			{ hash: 'a' },
			{ hash: 'b', author: PEER },
			{ hash: 'c' },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b'], ['c']]);
	});

	it('splits by timeframe', () => {
		const input = events([
			{ hash: 'a', offsetMs: 0 },
			{ hash: 'b', offsetMs: MESSAGE_SET_TIMEFRAME_INTERVAL_MS + 1 },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b']]);
	});

	it('splits by event type', () => {
		const input = events([
			{ hash: 'a' },
			{ hash: 'b', type: 'GroupControl' },
			{ hash: 'c' },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b'], ['c']]);
	});

	it('orders by lamport before timestamp', () => {
		// The peer's clock runs behind: their reply to 'a' carries an earlier
		// wall-clock timestamp but a higher lamport, and must still sort after.
		const input = events([
			{ hash: 'a', offsetMs: 0, lamport: 1 },
			{ hash: 'b', author: PEER, offsetMs: -120_000, lamport: 2 },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b']]);
	});

	it('tie-breaks equal lamports by timestamp', () => {
		const input = events([
			{ hash: 'b', author: PEER, offsetMs: 1000, lamport: 1 },
			{ hash: 'a', offsetMs: 0, lamport: 1 },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b']]);
	});

	it('anchors lamport-less events at their wall-clock position', () => {
		// 'block' carries no lamport (0); it must not sort before every
		// lamport-stamped message but stay between its wall-clock neighbours.
		const input = events([
			{ hash: 'a', offsetMs: 0, lamport: 1 },
			{ hash: 'block', offsetMs: 1000, type: 'Block' },
			{ hash: 'b', offsetMs: 2000, lamport: 2 },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['block'], ['b']]);
	});

	it('keeps day headers monotonic under clock skew', () => {
		// 'b' was stamped the previous day by a skewed clock but follows 'a'
		// causally; it must land in 'a's day group, not open an earlier day
		// after a later one.
		const dayMs = 24 * 60 * 60 * 1000;
		const input = events([
			{ hash: 'a', offsetMs: 0, lamport: 1 },
			{ hash: 'b', author: PEER, offsetMs: -dayMs, lamport: 2 },
		]);
		const days = groupEventsInDays(input, AGENT_SETS);
		assert.equal(days.length, 1);
		assert.deepEqual(groupHashes(input), [['a'], ['b']]);
	});
});
