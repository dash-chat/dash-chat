import { hashCode } from '$lib/utils/hash';

import { TextAvatarData } from './text-avatar-data-url';

export const TEXT_AVATAR_TEXT_COLOR = '#831843';

export const DEFAULT_TEXT_AVATAR_COLOR = '#fce7f3';

export const TEXT_AVATAR_COLORS = [
	'#ddd6fe',
	'#bfdbfe',
	'#cffafe',
	'#bbf7d0',
	'#e9d5ff',
	'#fbcfe8',
	DEFAULT_TEXT_AVATAR_COLOR,
	'#fecaca',
	'#fef08a',
	'#d9f99d',
	'#e5e7eb',
	'#d1d5db',
];

/** Stable text-avatar color for a user or chat, hashed from its id so every
 * device assigns the same color without coordination. */
export function defaultAvatarColor(seed: string): string {
	return TEXT_AVATAR_COLORS[hashCode(seed) % TEXT_AVATAR_COLORS.length];
}

/** Initials for a display name, following Signal's convention: the first
 * grapheme of the first word plus the first grapheme of the second word. */
export function abbreviateName(name: string): string {
	const words = name.split(/\s+/).filter(word => word.length > 0);
	return words.slice(0, 2).map(firstGrapheme).join('');
}

function firstGrapheme(word: string): string {
	if (typeof Intl !== 'undefined' && Intl.Segmenter) {
		const segments = new Intl.Segmenter().segment(word);
		for (const segment of segments) {
			return segment.segment;
		}
	}
	return String.fromCodePoint(word.codePointAt(0)!);
}

/** Initial editor state for a user with no stored avatar: their assigned
 * color and initials. Initials that can't be stored as a text avatar
 * (non-ASCII) are left empty rather than producing an unsaveable state. */
export function editorPrefill(
	name: string,
	surname: string | undefined,
	seed: string | undefined,
): TextAvatarData {
	const displayName = surname ? `${name} ${surname}` : name;
	const text = abbreviateName(displayName).toUpperCase();
	return new TextAvatarData(
		defaultAvatarColor(seed || displayName),
		TextAvatarData.isValidText(text) ? text : '',
	);
}
