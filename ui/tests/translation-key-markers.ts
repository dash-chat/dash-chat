const MARKER_EDGE = '⁠';
const MARKER_BITS = ['‌', '‍'];
const BITS_PER_CHAR = 7;
const MARKER = new RegExp(
	`${MARKER_EDGE}([${MARKER_BITS.join('')}]+)${MARKER_EDGE}`,
	'g',
);

/** `text` followed by an invisible encoding of its message `key`. */
export function markTranslationKey(key: string, text: string): string {
	const bits = [...key]
		.map(char => char.charCodeAt(0).toString(2).padStart(BITS_PER_CHAR, '0'))
		.join('');
	const marker = [...bits].map(bit => MARKER_BITS[Number(bit)]).join('');
	return `${text}${MARKER_EDGE}${marker}${MARKER_EDGE}`;
}

/** The message keys whose markers `text` carries. */
export function markedTranslationKeys(text: string): string[] {
	return [...text.matchAll(MARKER)].map(([, marker]) => {
		const bits = [...marker].map(char => MARKER_BITS.indexOf(char)).join('');
		const chars = bits.match(new RegExp(`.{${BITS_PER_CHAR}}`, 'g')) ?? [];
		return chars.map(char => String.fromCharCode(parseInt(char, 2))).join('');
	});
}
