import { markedTranslationKeys } from './markers.ts';

const TRANSLATED_ATTRIBUTES = ['placeholder', 'aria-label', 'title', 'alt'];

function isPaintedInViewport(el: Element): boolean {
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
}

/** The text and translated attributes `el` itself holds, without its descendants'. */
function ownTranslatedText(el: Element): string {
	const ownText = Array.from(el.childNodes)
		.filter(node => node.nodeType === Node.TEXT_NODE)
		.map(node => node.textContent);
	const attributes = TRANSLATED_ATTRIBUTES.map(name => el.getAttribute(name));
	// A closed <select> paints none of its options, yet they are what it offers.
	const options =
		el instanceof HTMLSelectElement
			? Array.from(el.options).map(option => option.textContent)
			: [];
	return [...ownText, ...attributes, ...options].join('');
}

export interface VisibleString {
	key: string;
	/** Where the element showing it is, to tell when it has stopped moving. */
	left: number;
	top: number;
}

/** The marked strings currently on screen. None unless the build marks its
 * messages. */
export function visibleStrings(): VisibleString[] {
	return Array.from(document.body.querySelectorAll('*'))
		.map(el => ({ el, keys: markedTranslationKeys(ownTranslatedText(el)) }))
		.filter(({ el, keys }) => keys.length > 0 && isPaintedInViewport(el))
		.flatMap(({ el, keys }) => {
			const { left, top } = el.getBoundingClientRect();
			return keys.map(key => ({ key, left, top }));
		});
}
