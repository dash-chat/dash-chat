import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { foldMovedMessages } from '../src/chats/moved-messages';
import type { SimplifiedOperation } from '../src/p2panda/simplified-types';
import type { DeviceId, Hash } from '../src/p2panda/types';
import type { MessageContent, Payload } from '../src/types';

const LOSER: DeviceId = 'loser';
const WINNER: DeviceId = 'winner';

function message(
	hash: Hash,
	author: DeviceId,
	timestamp: number,
	content: Partial<MessageContent> = {},
): SimplifiedOperation<Payload> {
	return {
		hash,
		header: {
			verifying_key: author,
			timestamp,
			seq_num: 0,
			backlink: undefined,
			previous: [],
			topic_id: 'topic',
			log_id: 'log',
			auth: undefined,
		},
		body: {
			type: 'Chat',
			payload: {
				type: 'Message',
				payload: { message: hash, media: null, ...content },
			},
		},
	};
}

function hashes(ops: SimplifiedOperation<Payload>[]): Hash[] {
	return ops.map(op => op.hash);
}

describe('foldMovedMessages', () => {
	it('shows a moved message once, at the time it was first written', () => {
		const ops = [
			message('original', LOSER, 1_000),
			message('reply', WINNER, 2_000, { reply: 'original' }),
			message('copy', LOSER, 3_000, {
				moved: { hash: 'original', timestamp: 1_000_000 },
			}),
		];

		const folded = foldMovedMessages(ops);

		assert.deepEqual(hashes(folded), ['copy', 'reply']);
		assert.equal(folded[0].header.timestamp, 1_000);
		const reply = folded[1].body;
		assert.ok(reply?.type === 'Chat' && reply.payload.type === 'Message');
		assert.equal(reply.payload.payload.reply, 'copy');
	});

	it('keeps a copy whose original is already gone', () => {
		const ops = [
			message('copy', LOSER, 3_000, {
				moved: { hash: 'original', timestamp: 1_000_000 },
			}),
		];

		const folded = foldMovedMessages(ops);

		assert.deepEqual(hashes(folded), ['copy']);
		assert.equal(folded[0].header.timestamp, 1_000);
	});

	it("does not let a copy stand in for someone else's message", () => {
		const ops = [
			message('theirs', WINNER, 1_000),
			message('copy', LOSER, 3_000, {
				moved: { hash: 'theirs', timestamp: 1_000_000 },
			}),
		];

		assert.deepEqual(hashes(foldMovedMessages(ops)), ['theirs', 'copy']);
	});
});
