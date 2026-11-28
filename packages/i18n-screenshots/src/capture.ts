import type { Page } from 'playwright-core';

import { MARKER } from './markers.ts';

/** The picture of a page and what it shows. */
export interface Screenshot {
	/** The picture, as PNG. */
	image: Uint8Array;
	/** The keys of the marked strings it shows. */
	keys: string[];
}

interface OnThePage {
	/** Each marked string on screen, with where it is. */
	strings: string[];
	fontsLoaded: boolean;
}

/** Runs in the page, so it can use nothing from outside itself but
 * `marker`. */
function whatIsOnThePage(marker: typeof MARKER): OnThePage {
	const marked = new RegExp(
		`${marker.edge}([${marker.bits.join('')}]+)${marker.edge}`,
		'g',
	);
	const keysIn = (text: string) =>
		[...text.matchAll(marked)].map(([, written]) => {
			const bits = [...written].map(char => marker.bits.indexOf(char)).join('');
			const chars =
				bits.match(new RegExp(`.{${marker.bitsPerChar}}`, 'g')) ?? [];
			return chars.map(char => String.fromCharCode(parseInt(char, 2))).join('');
		});
	/** The text and translated attributes `el` itself holds, without its
	 * descendants'. */
	const ownTranslatedText = (el: Element) =>
		[
			...Array.from(el.childNodes)
				.filter(node => node.nodeType === Node.TEXT_NODE)
				.map(node => node.textContent),
			...['placeholder', 'aria-label', 'title', 'alt'].map(name =>
				el.getAttribute(name),
			),
			// A closed <select> paints none of its options, yet they are what it offers.
			...(el instanceof HTMLSelectElement
				? Array.from(el.options).map(option => option.textContent)
				: []),
		].join('');
	const isPaintedInViewport = (el: Element) => {
		const box = el.getBoundingClientRect();
		return (
			box.width > 0 &&
			box.height > 0 &&
			box.bottom > 0 &&
			box.right > 0 &&
			box.top < window.innerHeight &&
			box.left < window.innerWidth &&
			el.checkVisibility({ opacityProperty: true, visibilityProperty: true })
		);
	};
	const strings = Array.from(document.body.querySelectorAll('*')).flatMap(
		el => {
			const keys = keysIn(ownTranslatedText(el));
			if (keys.length === 0 || !isPaintedInViewport(el)) return [];
			const { left, top } = el.getBoundingClientRect();
			return keys.map(key => `${key}@${left},${top}`);
		},
	);
	return { strings, fontsLoaded: document.fonts.status === 'loaded' };
}

const BETWEEN_LOOKS_MS = 100;

/** The marked strings on a page that has rendered and stopped changing:
 * while it loads, a page holds still with nothing on it, and what scrolls or
 * slides into place is not there yet. */
async function settledStrings(page: Page): Promise<string[]> {
	let before = await page.evaluate(whatIsOnThePage, MARKER);
	for (;;) {
		await page.waitForTimeout(BETWEEN_LOOKS_MS);
		const now = await page.evaluate(whatIsOnThePage, MARKER);
		const settled =
			now.fontsLoaded &&
			now.strings.length > 0 &&
			now.strings.join() === before.strings.join();
		if (settled) return now.strings;
		before = now;
	}
}

/** Take a picture of the page once it has settled, with the keys of the
 * marked strings on it. The page may still change between the two being
 * taken, so a picture counts only when the strings were the same right
 * before and right after it. */
export async function takeScreenshot(page: Page): Promise<Screenshot> {
	let strings = await settledStrings(page);
	for (;;) {
		// A screenshot taken mid-transition shows neither state.
		const image = await page.screenshot({ animations: 'disabled' });
		const stringsAfter = await settledStrings(page);
		if (stringsAfter.join() === strings.join()) {
			const keys = strings.map(string =>
				string.slice(0, string.lastIndexOf('@')),
			);
			return { image, keys: [...new Set(keys)].sort() };
		}
		strings = stringsAfter;
	}
}
