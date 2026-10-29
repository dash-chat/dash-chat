import type { DeviceId, Hash, Message } from 'dash-chat-stores';
import { describe, expect, it } from 'vitest';

import { closestDay, messageTextHtml } from './message-helpers';
import {
	searchMatchRanges,
	searchMessages,
	searchTerms,
	textMatchesSearch,
} from './message-search';

function matches(text: string, query: string): boolean {
	return textMatchesSearch(text, searchTerms(query));
}

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

describe('textMatchesSearch', () => {
	it('matches a query word against the start of a word', () => {
		expect(matches('see you at the station', 'sta')).toBe(true);
	});

	it('does not match inside a word', () => {
		expect(matches('see you at the station', 'tion')).toBe(false);
	});

	it('needs every query word to start some word, in any order', () => {
		expect(matches('see you at the station around nine', 'nine sta')).toBe(
			true,
		);
		expect(matches('see you at the station', 'sta ten')).toBe(false);
	});

	it('ignores case and accents', () => {
		expect(matches('Meet at the Café', 'cafe')).toBe(true);
		expect(matches('meet at the cafe', 'CAFÉ')).toBe(true);
	});

	it('ignores punctuation in the query and the text', () => {
		expect(matches('ok, see you!', 'you?')).toBe(true);
	});

	it('matches emoji', () => {
		expect(matches('great news 🎉', '🎉')).toBe(true);
	});

	it('matches nothing for a query with no words', () => {
		expect(matches('anything', '  ')).toBe(false);
		expect(matches('anything', '?!')).toBe(false);
	});
});

describe('searchMessages', () => {
	it('returns the matching messages oldest first', () => {
		const days = [
			day(message('a', 'the station'), message('b', 'no')),
			day(message('c', 'station again')),
		];
		expect(searchMessages(days, 'station')).toEqual(['a', 'c']);
	});

	it('skips deleted messages', () => {
		const deleted: Message = {
			...message('a', ''),
			content: 'deleted-for-everyone',
		};
		expect(searchMessages([day(deleted)], 'station')).toEqual([]);
	});

	it('does not match a reply by the text it quotes', () => {
		const reply: Message = {
			...message('b', 'sure'),
			replyQuote: {
				kind: 'content',
				author: 'device' as DeviceId,
				text: 'the station',
				media: null,
			},
		};
		expect(
			searchMessages([day(message('a', 'the station'), reply)], 'station'),
		).toEqual(['a']);
	});

	it('skips events that are not messages', () => {
		const days = [
			{
				eventsGroups: [[['r', { kind: 'report' }] as [Hash, { kind: string }]]],
			},
		];
		expect(searchMessages(days, 'report')).toEqual([]);
	});
});

describe('searchMatchRanges', () => {
	it('covers the matched start of each word', () => {
		expect(
			searchMatchRanges('stay at the station', searchTerms('sta')),
		).toEqual([
			[0, 3],
			[12, 15],
		]);
	});

	it('covers the longest query word a word starts with', () => {
		expect(searchMatchRanges('station', searchTerms('st stat'))).toEqual([
			[0, 4],
		]);
	});

	it('covers the original characters when the match ignored an accent', () => {
		expect(searchMatchRanges('Café au lait', searchTerms('cafe'))).toEqual([
			[0, 4],
		]);
	});
});

describe('messageTextHtml', () => {
	it('highlights word starts only', () => {
		expect(messageTextHtml('station tion', 'tion')).toBe(
			'station <mark class="search-highlight">tion</mark>',
		);
	});

	it('highlights inside a link without breaking it', () => {
		expect(messageTextHtml('go to https://example.com now', 'example')).toBe(
			'go to <a href="https://example.com" class="message-link" data-testid="message-link">https://<mark class="search-highlight">example</mark>.com</a> now',
		);
	});

	it('escapes the text around a highlight', () => {
		expect(messageTextHtml('<b>bold</b>', 'bold')).toBe(
			'&lt;b&gt;<mark class="search-highlight">bold</mark>&lt;/b&gt;',
		);
	});
});

describe('closestDay', () => {
	it('picks the day nearest the target', () => {
		const days = [new Date(2026, 0, 1), new Date(2026, 0, 10)];
		expect(closestDay(days, new Date(2026, 0, 7))).toEqual(days[1]);
		expect(closestDay([], new Date())).toBeUndefined();
	});
});
