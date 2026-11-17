import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { foldMovedMessages } from '../src/chats/moved-messages';
import type { SimplifiedOperation } from '../src/p2panda/simplified-types';
import type { DeviceId, Hash } from '../src/p2panda/types';
import type { ChatId, MessageContent, Payload } from '../src/types';

const LOSER: DeviceId = 'loser';
const WINNER: DeviceId = 'winner';
const LOSING: ChatId = 'losing-space';
const WINNING: ChatId = 'winning-space';

function message(
	hash: Hash,
	author: DeviceId,
	space: ChatId,
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
			topic_id: space,
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
	it('shows a moved message once, as its copy in the space settled on', () => {
		const ops = [
			message('original', LOSER, LOSING, 1_000),
			message('copy', LOSER, WINNING, 1_000),
			message('reply', WINNER, WINNING, 2_000, { reply: 'original' }),
		];

		const folded = foldMovedMessages(ops, WINNING);

		assert.deepEqual(hashes(folded), ['copy', 'reply']);
		const reply = folded[1].body;
		assert.ok(reply?.type === 'Chat' && reply.payload.type === 'Message');
		assert.equal(reply.payload.payload.reply, 'copy');
	});

	it('keeps a message that has not been moved yet', () => {
		const ops = [
			message('moved', LOSER, LOSING, 1_000),
			message('copy', LOSER, WINNING, 1_000),
			message('pending', LOSER, LOSING, 2_000),
		];

		assert.deepEqual(hashes(foldMovedMessages(ops, WINNING)), [
			'copy',
			'pending',
		]);
	});

	it("does not take someone else's message of the same date for a copy", () => {
		const ops = [
			message('theirs', LOSER, LOSING, 1_000),
			message('mine', WINNER, WINNING, 1_000),
		];

		assert.deepEqual(hashes(foldMovedMessages(ops, WINNING)), [
			'theirs',
			'mine',
		]);
	});
});
