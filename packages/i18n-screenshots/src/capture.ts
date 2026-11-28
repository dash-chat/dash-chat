import type { Page } from 'playwright-core';

import { MARKER } from './markers.ts';

/** The picture of a page and what it shows. */
export interface Screenshot {
	/** The picture, as PNG. */
	image: Uint8Array;
	/** The keys of the marked strings it shows. */
	keys: string[];
}

/** Runs in the page, so it can use nothing from outside itself but
 * `marker`. */
function keysOnThePage(marker: typeof MARKER): string[] {
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
	return Array.from(document.body.querySelectorAll('*'))
		.filter(isPaintedInViewport)
		.flatMap(el => keysIn(ownTranslatedText(el)));
}

/** The keys of the marked strings the page shows. */
export async function shownKeys(page: Page): Promise<string[]> {
	const keys = await page.evaluate(keysOnThePage, MARKER);
	return [...new Set(keys)].sort();
}
