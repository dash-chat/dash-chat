import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	dropOpsAuthoredSinceLeaving,
	dropOpsAuthoredWhileNotAMember,
} from '../src/chats/op-visibility';
import type {
	GroupAction,
	SimplifiedOperation,
} from '../src/p2panda/simplified-types';
import type { DeviceId, Hash } from '../src/p2panda/types';
import type { ChatPayload, Payload } from '../src/types';

const ME: DeviceId = 'me';
const ADMIN: DeviceId = 'admin';
const OTHER: DeviceId = 'other';

function header(author: DeviceId, timestamp: number) {
	return {
		verifying_key: author,
		timestamp,
		seq_num: 0,
		backlink: undefined,
		previous: [],
		topic_id: 'topic',
	};
}

function chatOp(
	hash: Hash,
	timestamp: number,
	payload: ChatPayload,
): SimplifiedOperation<Payload> {
	return {
		hash,
		header: header(ADMIN, timestamp),
		body: { type: 'Chat', payload },
	};
}

function message(hash: Hash, timestamp: number): SimplifiedOperation<Payload> {
	return chatOp(hash, timestamp, {
		type: 'Message',
		payload: { message: hash, media: null, reply: undefined, lamport: 0 },
	});
}

function rename(hash: Hash, timestamp: number): SimplifiedOperation<Payload> {
	return chatOp(hash, timestamp, {
		type: 'GroupInfo',
		payload: { name: hash, description: undefined, image: undefined },
	});
}

function control(
	hash: Hash,
	timestamp: number,
	action: GroupAction,
): SimplifiedOperation<Payload> {
	return {
		hash,
		header: {
			...header(ADMIN, timestamp),
			auth: { group_id: 'group', action, dependencies: [] },
		},
		body: undefined,
	};
}

const create = (timestamp: number) =>
	control('create', timestamp, {
		Create: {
			initial_members: [
				[{ Individual: ADMIN }, null],
				[{ Individual: ME }, null],
			],
		},
	});
const add = (hash: Hash, timestamp: number, member: DeviceId) =>
	control(hash, timestamp, {
		Add: { member: { Individual: member }, access: null },
	});
const remove = (hash: Hash, timestamp: number, member: DeviceId) =>
	control(hash, timestamp, { Remove: { member: { Individual: member } } });

const hashes = (ops: SimplifiedOperation<Payload>[]) => ops.map(op => op.hash);

describe('dropOpsAuthoredWhileNotAMember', () => {
	it('leaves a chat that never names us untouched', () => {
		const ops = [message('m1', 1), add('add-other', 2, OTHER)];
		assert.deepEqual(dropOpsAuthoredWhileNotAMember(ops, ops, ME), ops);
	});

	it('hides everything after our removal but the removal itself', () => {
		const ops = [
			create(1),
			message('before', 2),
			remove('remove-me', 3, ME),
			message('after', 4),
			add('add-other', 5, OTHER),
			rename('renamed', 6),
		];
		assert.deepEqual(hashes(dropOpsAuthoredWhileNotAMember(ops, ops, ME)), [
			'create',
			'before',
			'remove-me',
		]);
	});

	it('shows what follows being added back, and still not what was missed', () => {
		const ops = [
			create(1),
			remove('remove-me', 2, ME),
			message('missed', 3),
			add('add-other', 4, OTHER),
			add('add-me', 5, ME),
			message('back', 6),
			remove('remove-other', 7, OTHER),
		];
		assert.deepEqual(hashes(dropOpsAuthoredWhileNotAMember(ops, ops, ME)), [
			'create',
			'remove-me',
			'add-me',
			'back',
			'remove-other',
		]);
	});

	it('hides an op stamped at the very moment of our removal', () => {
		const ops = [create(1), remove('remove-me', 2, ME), message('same', 2)];
		assert.deepEqual(hashes(dropOpsAuthoredWhileNotAMember(ops, ops, ME)), [
			'create',
			'remove-me',
		]);
	});
});

describe('dropOpsAuthoredSinceLeaving', () => {
	it('keeps a rename made after we left out of sight', () => {
		const ops = [
			create(1),
			rename('old', 2),
			remove('remove-me', 3, ME),
			rename('new', 4),
		];
		assert.deepEqual(hashes(dropOpsAuthoredSinceLeaving(ops, ops, ME)), [
			'create',
			'old',
		]);
	});

	it('catches up on a rename we missed once we are added back', () => {
		const ops = [
			create(1),
			rename('old', 2),
			remove('remove-me', 3, ME),
			rename('new', 4),
			add('add-me', 5, ME),
		];
		assert.deepEqual(dropOpsAuthoredSinceLeaving(ops, ops, ME), ops);
	});
});
