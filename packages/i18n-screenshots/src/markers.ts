/** The invisible characters a key is written in after its string: an edge on
 * each side of a run of the two bits. */
export const MARKER = { edge: '⁠', bits: ['‌', '‍'], bitsPerChar: 7 };

/** `text` followed by an invisible encoding of its message `key`. */
function markTranslationKey(key: string, text: string): string {
	const bits = [...key]
		.map(char =>
			char.charCodeAt(0).toString(2).padStart(MARKER.bitsPerChar, '0'),
		)
		.join('');
	const marker = [...bits].map(bit => MARKER.bits[Number(bit)]).join('');
	return `${text}${MARKER.edge}${marker}${MARKER.edge}`;
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
