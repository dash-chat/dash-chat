import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import type { DeviceId, Hash } from '../src/p2panda/types';
import type { MessageDeliveryStatus } from '../src/types';
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
		deliveryStatus?: MessageDeliveryStatus;
	}>,
): Record<Hash, EventWithProvenance<TestEvent>> {
	const out: Record<Hash, EventWithProvenance<TestEvent>> = {};
	for (let i = 0; i < specs.length; i++) {
		const spec = specs[i];
		out[spec.hash] = {
			event: { hash: spec.hash },
			author: spec.author ?? ME,
			timestamp: BASE_TS + (spec.offsetMs ?? i * 1000),
			type: 'Message',
			deliveryStatus: spec.deliveryStatus,
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

describe('groupEventsInDays delivery-status splitting', () => {
	it('keeps messages with the same delivery status in one group', () => {
		const input = events([
			{ hash: 'a', deliveryStatus: 'delivered' },
			{ hash: 'b', deliveryStatus: 'delivered' },
			{ hash: 'c', deliveryStatus: 'delivered' },
		]);
		assert.deepEqual(groupHashes(input), [['a', 'b', 'c']]);
	});

	it('splits a group wherever the delivery status changes', () => {
		const input = events([
			{ hash: 'a', deliveryStatus: 'delivered' },
			{ hash: 'b', deliveryStatus: 'delivered' },
			{ hash: 'c', deliveryStatus: 'mailbox' },
			{ hash: 'd', deliveryStatus: 'sending' },
			{ hash: 'e', deliveryStatus: 'sending' },
		]);
		assert.deepEqual(groupHashes(input), [['a', 'b'], ['c'], ['d', 'e']]);
	});

	it('regroups once statuses converge again', () => {
		const specs = [
			{ hash: 'a', deliveryStatus: 'delivered' as const },
			{ hash: 'b', deliveryStatus: 'mailbox' as const },
			{ hash: 'c', deliveryStatus: 'sending' as const },
		];
		assert.deepEqual(groupHashes(events(specs)), [['a'], ['b'], ['c']]);

		const converged = specs.map(spec => ({
			...spec,
			deliveryStatus: 'delivered' as const,
		}));
		assert.deepEqual(groupHashes(events(converged)), [['a', 'b', 'c']]);
	});

	it('keeps events without a delivery status in one group', () => {
		const input = events([
			{ hash: 'a', author: PEER },
			{ hash: 'b', author: PEER },
		]);
		assert.deepEqual(groupHashes(input), [['a', 'b']]);
	});

	it('still splits by author when statuses match', () => {
		const input = events([
			{ hash: 'a', deliveryStatus: 'delivered' },
			{ hash: 'b', author: PEER },
			{ hash: 'c', deliveryStatus: 'delivered' },
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b'], ['c']]);
	});

	it('still splits by timeframe when statuses match', () => {
		const input = events([
			{ hash: 'a', offsetMs: 0, deliveryStatus: 'delivered' },
			{
				hash: 'b',
				offsetMs: MESSAGE_SET_TIMEFRAME_INTERVAL_MS + 1,
				deliveryStatus: 'delivered',
			},
		]);
		assert.deepEqual(groupHashes(input), [['a'], ['b']]);
	});
});
