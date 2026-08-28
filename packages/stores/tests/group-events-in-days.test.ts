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
});
