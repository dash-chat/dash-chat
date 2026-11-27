const MARKER_EDGE = '⁠';
const MARKER_BITS = ['‌', '‍'];
const BITS_PER_CHAR = 7;
const MARKER = new RegExp(
	`${MARKER_EDGE}([${MARKER_BITS.join('')}]+)${MARKER_EDGE}`,
	'g',
);

/** `text` followed by an invisible encoding of its message `key`. */
function markTranslationKey(key: string, text: string): string {
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

/** `sourceStrings` with each one followed by the marker of the key the app
 * renders it under, for an app to be built from in place of the originals:
 * whatever it then shows says which string it is. `renderedKey` gives that
 * key where it is not the one the string is filed under. */
export function markedSourceStrings(
	sourceStrings: Record<string, string>,
	renderedKey: (sourceKey: string) => string = sourceKey => sourceKey,
): Record<string, string> {
	return Object.fromEntries(
		Object.entries(sourceStrings).map(([sourceKey, text]) => [
			sourceKey,
			markTranslationKey(renderedKey(sourceKey), text),
		]),
	);
}
