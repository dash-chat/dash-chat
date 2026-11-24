import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { logsToMessages, opsInCausalOrder } from '../src/chats/messages-store';
import type { SimplifiedOperation } from '../src/p2panda/simplified-types';
import type { DeviceId, Hash } from '../src/p2panda/types';
import { type ChatPayload, type Payload, hasBody } from '../src/types';

const FAST_CLOCK: DeviceId = 'fast-clock';
const SLOW_CLOCK: DeviceId = 'slow-clock';
const BASE_TS = new Date(2026, 0, 5, 12, 0, 0).valueOf();
const DEVICE_AGENTS = {
	[FAST_CLOCK]: 'fast-agent',
	[SLOW_CLOCK]: 'slow-agent',
};
const DEVICE_NAMES = { [FAST_CLOCK]: 'Fast', [SLOW_CLOCK]: 'Slow' };

function chatOp(
	hash: Hash,
	author: DeviceId,
	timestamp: number,
	payload: ChatPayload,
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
		},
		body: { type: 'Chat', payload },
	};
}

function message(
	hash: Hash,
	author: DeviceId,
	timestamp: number,
	lamport: number,
	reply?: Hash,
): SimplifiedOperation<Payload> {
	return chatOp(hash, author, timestamp, {
		type: 'Message',
		payload: { message: `text of ${hash}`, media: null, reply, lamport },
	});
}

describe('logsToMessages under clock skew', () => {
	it('keeps the quote of a reply stamped before its target', () => {
		const messages = logsToMessages(
			opsInCausalOrder([
				message('reply', SLOW_CLOCK, BASE_TS, 2, 'target'),
				message('target', FAST_CLOCK, BASE_TS + 500, 1),
			]),
			{},
			DEVICE_AGENTS,
			DEVICE_NAMES,
		);

		assert.deepEqual(messages.reply.replyQuote, {
			kind: 'content',
			author: FAST_CLOCK,
			authorName: 'Fast',
			text: 'text of target',
			media: null,
			scrollTarget: 'target',
		});
	});

	it('keeps a reaction stamped before its target', () => {
		const messages = logsToMessages(
			opsInCausalOrder([
				chatOp('reaction', SLOW_CLOCK, BASE_TS, {
					type: 'Reaction',
					payload: { emoji: '👍', target: 'target' },
				}),
				message('target', FAST_CLOCK, BASE_TS + 500, 1),
			]),
			{},
			DEVICE_AGENTS,
			DEVICE_NAMES,
		);

		const { content } = messages.target;
		assert.ok(hasBody(content));
		assert.deepEqual(content.reactions, { 'slow-agent': '👍' });
	});

	it('freezes the quote at the text the target had when replying', () => {
		const messages = logsToMessages(
			opsInCausalOrder([
				message('target', FAST_CLOCK, BASE_TS, 1),
				message('reply', SLOW_CLOCK, BASE_TS + 100, 2, 'target'),
				chatOp('edit', FAST_CLOCK, BASE_TS + 200, {
					type: 'EditMessage',
					payload: { message: 'edited', edit_hash: 'target' },
				}),
			]),
			{},
			DEVICE_AGENTS,
			DEVICE_NAMES,
		);

		const { replyQuote } = messages.reply;
		assert.equal(replyQuote?.kind, 'content');
		assert.equal(replyQuote?.text, 'text of target');
	});
});
