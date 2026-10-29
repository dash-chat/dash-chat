import type { DeviceId, Hash, Message } from 'dash-chat-stores';
import { describe, expect, it } from 'vitest';

import { closestDay } from './message-helpers';
import { searchMessages } from './message-search';

function message(hash: Hash, text: string): Message {
	return {
		hash,
		content: { message: text, media: null, reactions: {}, editHistory: [] },
		timestamp: 0,
		author: 'device' as DeviceId,
		seqNum: 0,
	};
}

type MessageEvent = { kind: string; message: Message };

function day(...messages: Message[]) {
	return {
		eventsGroups: [
			messages.map(
				m => [m.hash, { kind: 'message', message: m }] as [Hash, MessageEvent],
			),
		],
	};
}

describe('searchMessages', () => {
	it('finds messages that are not rendered, oldest first, ignoring case', () => {
		const days = [
			day(message('a' as Hash, 'See you at the Station')),
			day(message('b' as Hash, 'no'), message('c' as Hash, 'station 2')),
		];
		expect(searchMessages(days, 'station')).toEqual(['a', 'c']);
	});

	it('finds nothing for an empty query', () => {
		expect(searchMessages([day(message('a' as Hash, 'hi'))], '')).toEqual([]);
	});
});

describe('closestDay', () => {
	it('picks the day nearest the target', () => {
		const days = [new Date(2026, 0, 1), new Date(2026, 0, 10)];
		expect(closestDay(days, new Date(2026, 0, 7))).toEqual(days[1]);
		expect(closestDay([], new Date())).toBeUndefined();
	});
});
