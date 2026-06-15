import { hashCode } from '$lib/utils/hash';

import { TextAvatarData } from './text-avatar-data-url';

export const TEXT_AVATAR_TEXT_COLOR = '#831843';

export const DEFAULT_TEXT_AVATAR_COLOR = '#fce7f3';

// The greys stay pickable in the editor but are never auto-assigned.
const ASSIGNABLE_COLORS = [
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
];

export const TEXT_AVATAR_COLORS = [...ASSIGNABLE_COLORS, '#e5e7eb', '#d1d5db'];

/** Stable text-avatar color for a user or chat, hashed from its id so every
 * device assigns the same color without coordination. */
export function defaultAvatarColor(seed: string): string {
	return ASSIGNABLE_COLORS[hashCode(seed) % ASSIGNABLE_COLORS.length];
}

/** Display name from the raw profile fields, like [fullName] but usable
 * before a Profile exists. */
export function joinName(
	name: string | undefined,
	surname: string | undefined,
): string | undefined {
	if (!name) return undefined;
	return surname ? `${name} ${surname}` : name;
}

/** Initials for a display name, matching Signal's `getInitials`: strip
 * everything that isn't a letter or separator, then take the first grapheme of
 * the first word plus the first grapheme of the last word (just the first
 * grapheme for a single-word name), preserving the name's case. A cleaned name
 * that is already a two-letter all-caps abbreviation is returned as-is. */
export function abbreviateName(name: string): string {
	const cleaned = name
		.replace(/[^\p{L}\p{Z}]+/gu, '')
		.replace(/\p{Z}+/gu, ' ')
		.trim();
	if (!cleaned) return '';
	if (cleaned.length === 2 && cleaned === cleaned.toUpperCase()) {
		return cleaned;
	}
	const words = cleaned.split(' ');
	return words.length === 1
		? firstGrapheme(words[0])
		: firstGrapheme(words[0]) + firstGrapheme(words[words.length - 1]);
}

const segmenter =
	typeof Intl !== 'undefined' && Intl.Segmenter
		? new Intl.Segmenter()
		: undefined;

function firstGrapheme(word: string): string {
	if (segmenter) {
		for (const segment of segmenter.segment(word)) {
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
	const displayName = joinName(name, surname) ?? '';
	// The stored format only accepts upper-case, unlike the virtual default.
	const text = abbreviateName(displayName).toUpperCase();
	return new TextAvatarData(
		defaultAvatarColor(seed || displayName),
		TextAvatarData.isValidText(text) ? text : '',
	);
}

export interface InitialsLayout {
	/** Font size in px, shrunk from the base 0.45·diameter if the label is too
	 * wide/tall to keep Signal's ~10% margin. */
	fontSizePx: number;
	/** Vertical nudge in px (positive = down) that recentres the measured ink
	 * box, given that `text-box: trim-both cap alphabetic` parks the baseline
	 * half a cap-height below the circle's centre. */
	translateYPx: number;
}

const layoutCache = new Map<string, InitialsLayout>();
let measureCtx: CanvasRenderingContext2D | null | undefined;

function measureContext(): CanvasRenderingContext2D | null {
	if (measureCtx === undefined) {
		measureCtx =
			typeof document !== 'undefined'
				? document.createElement('canvas').getContext('2d')
				: null;
	}
	return measureCtx;
}

/** Lay out a text avatar's initials like Signal's `AvatarBuilder`: measure the
 * real ink box, scale the label down so its larger axis fits within 0.8 of the
 * diameter (Signal's 10%-per-side margin), and report the vertical offset that
 * centres the measured box. Content-aware, so caps, lower-case, CJK and
 * descenders all land on the circle's true centre. */
export function measureInitialsLayout(
	text: string,
	diameterPx: number,
	fontFamily: string,
	fontWeight = 500,
): InitialsLayout {
	const baseFontPx = diameterPx * 0.45;
	const fallback: InitialsLayout = { fontSizePx: baseFontPx, translateYPx: 0 };
	const ctx = measureContext();
	if (!ctx || !text || diameterPx <= 0) {
		return fallback;
	}

	const key = `${fontWeight}|${diameterPx}|${fontFamily}|${text}`;
	const cached = layoutCache.get(key);
	if (cached) return cached;

	ctx.font = `${fontWeight} ${baseFontPx}px ${fontFamily}`;
	const base = ctx.measureText(text);
	const inkWidth = base.actualBoundingBoxLeft + base.actualBoundingBoxRight;
	const inkHeight =
		base.actualBoundingBoxAscent + base.actualBoundingBoxDescent;
	const largerAxis = Math.max(inkWidth, inkHeight);
	const scale =
		largerAxis > 0 ? Math.min(1, (diameterPx * 0.8) / largerAxis) : 1;
	const fontSizePx = baseFontPx * scale;

	ctx.font = `${fontWeight} ${fontSizePx}px ${fontFamily}`;
	const capHeight = ctx.measureText('H').actualBoundingBoxAscent;
	const ink = ctx.measureText(text);
	const inkCentreBelowBaseline =
		(ink.actualBoundingBoxDescent - ink.actualBoundingBoxAscent) / 2;
	const translateYPx = -(capHeight / 2 + inkCentreBelowBaseline);

	const layout: InitialsLayout = { fontSizePx, translateYPx };
	layoutCache.set(key, layout);
	return layout;
}
