import { type Hash, type Message, hasBody } from 'dash-chat-stores';

const wordSegmenter = new Intl.Segmenter(undefined, { granularity: 'word' });

interface Word {
	text: string;
	index: number;
}

/** Words as Signal's search index sees them: word-like segments, plus emoji,
 * which the segmenter does not count as words but people do search for. */
function words(text: string): Word[] {
	return Array.from(wordSegmenter.segment(text))
		.filter(s => s.isWordLike || /\p{Extended_Pictographic}/u.test(s.segment))
		.map(s => ({ text: s.segment, index: s.index }));
}

function fold(text: string): string {
	return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

/** The folded words of a search query; empty when it has nothing to search. */
export function searchTerms(query: string): string[] {
	return words(query).map(word => fold(word.text));
}

/** Whether every term starts some word of `text`, ignoring case and accents. */
export function textMatchesSearch(text: string, terms: string[]): boolean {
	if (terms.length === 0) return false;
	const folded = words(text).map(word => fold(word.text));
	return terms.every(term => folded.some(word => word.startsWith(term)));
}

/** The text a search looks at: a message's current text, which is also its
 * caption when it carries media. Deleted messages and the quote inside a reply
 * have none, so a reply is never a second hit for the message it quotes. */
function searchableText(message: Message): string {
	return hasBody(message.content) ? message.content.message : '';
}

/** The hashes of the messages matching `query`, oldest first. */
export function searchMessages<T extends { kind: string; message?: Message }>(
	days: Array<{ eventsGroups: Array<Array<[Hash, T]>> }>,
	query: string,
): Hash[] {
	const terms = searchTerms(query);
	if (terms.length === 0) return [];
	return days
		.flatMap(day => day.eventsGroups.flat())
		.filter(
			([, event]) =>
				event.message !== undefined &&
				textMatchesSearch(searchableText(event.message), terms),
		)
		.map(([hash]) => hash);
}

/** How many characters of `word` the folded `prefixLength` characters span,
 * so a highlight covers the original text even when folding changed it. */
function originalPrefixLength(word: string, prefixLength: number): number {
	let folded = 0;
	let length = 0;
	for (const char of word) {
		if (folded >= prefixLength && fold(char) !== '') break;
		folded += fold(char).length;
		length += char.length;
	}
	return length;
}

/** The `[start, end)` ranges of `text` that the search terms match: the
 * start of each word that a term begins, as long as the longest such term. */
export function searchMatchRanges(
	text: string,
	terms: string[],
): Array<[number, number]> {
	if (terms.length === 0) return [];
	return words(text).flatMap(word => {
		const folded = fold(word.text);
		const longest = Math.max(
			0,
			...terms.filter(term => folded.startsWith(term)).map(t => t.length),
		);
		if (longest === 0) return [];
		const end = word.index + originalPrefixLength(word.text, longest);
		return [[word.index, end] as [number, number]];
	});
}
