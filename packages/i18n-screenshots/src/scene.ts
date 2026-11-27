import { type VisibleString, visibleStrings } from './visible-keys.ts';

/** One state of the app worth showing a translator. */
export interface Scene<Device extends string = string> {
	/** Names the screenshot. */
	name: string;
	/** What to render it as; the config's default device unless said otherwise. */
	device?: Device;
	/** Keys of the strings it shows as pixels, which no marker can report:
	 * text the app draws into a picture. */
	drawn?: string[];
	/** Brings the app to that state. It starts as the app does, so what it does
	 * before it first awaits is in place before the app loads anything. */
	setup?: () => void | Promise<void>;
}

interface ScenesOnThePage {
	scenes: { name: string; device?: string }[];
	/** The keys of the strings the requested scene shows, once its setup is
	 * done and the page has stopped changing. */
	shown: () => Promise<string[]>;
}

declare global {
	interface Window {
		__i18nScreenshots?: ScenesOnThePage;
	}
}

/** Tell whoever takes the screenshots which `scenes` there are, and play the
 * one the `?scene=` query parameter asks for. Call it where the app starts,
 * before it renders. */
export function playRequestedScene(scenes: Scene[]): void {
	const requestedName = new URLSearchParams(location.search).get('scene');
	const requested = scenes.find(scene => scene.name === requestedName);
	if (requested !== undefined) stopAllMotion();
	const played = (async () => requested?.setup?.())();
	// Nobody may be waiting for it: a scene opened by hand in a browser.
	played.catch(error => console.error(`scene ${requested?.name}:`, error));
	window.__i18nScreenshots = {
		scenes: scenes.map(({ name, device }) => ({ name, device })),
		shown: async () => {
			await played;
			return [...(await keysOfTheLoadedApp()), ...(requested?.drawn ?? [])];
		},
	};
}

/** A screenshot taken mid-transition shows neither state. */
function stopAllMotion(): void {
	const style = document.createElement('style');
	style.textContent =
		'*, *::before, *::after { transition: none !important; animation: none !important; }';
	document.head.appendChild(style);
}

function twoFramesOn(): Promise<void> {
	return new Promise(resolve =>
		requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
	);
}

// One look alike can fall between two steps of the same change: a dialog
// that mounts, then moves into place under what is to be tapped next.
const LOOKS_UNCHANGED_TO_BE_SETTLED = 3;

function sameStringsInTheSamePlaces(
	before: VisibleString[],
	after: VisibleString[],
): boolean {
	return JSON.stringify(before) === JSON.stringify(after);
}

/** The message keys on screen once they have stopped changing and moving:
 * data arrives and dialogs mount a few frames after what triggered them, and
 * what scrolls into place is not there yet. */
async function settledKeys(): Promise<string[]> {
	let strings = visibleStrings();
	let looksInARowUnchanged = 0;
	while (looksInARowUnchanged < LOOKS_UNCHANGED_TO_BE_SETTLED) {
		await twoFramesOn();
		const next = visibleStrings();
		looksInARowUnchanged = sameStringsInTheSamePlaces(strings, next)
			? looksInARowUnchanged + 1
			: 0;
		strings = next;
	}
	return [...new Set(strings.map(({ key }) => key))].sort();
}

/** The settled keys of an app that has rendered: while it loads, the page
 * holds still with nothing on it. */
async function keysOfTheLoadedApp(): Promise<string[]> {
	for (;;) {
		const keys = await settledKeys();
		if (keys.length > 0 && document.fonts.status === 'loaded') return keys;
	}
}

/** Resolves once what the page shows has stopped changing. */
export async function settled(): Promise<void> {
	await settledKeys();
}

const NOTHING_TO_ACT_ON_AFTER_MS = 10_000;

/** What `look` finds once it finds something, for what a scene acts on:
 * the page gets to it some frames after what leads there. */
export async function eventually<E>(
	look: () => E | null | undefined,
	lookedFor: string,
): Promise<E> {
	const givenUpAt = Date.now() + NOTHING_TO_ACT_ON_AFTER_MS;
	for (;;) {
		const found = look();
		if (found !== null && found !== undefined) return found;
		if (Date.now() > givenUpAt) throw new Error(`${lookedFor} never showed up`);
		await twoFramesOn();
	}
}

/** The element with `testId` once there is one to see. */
export function find(testId: string): Promise<HTMLElement> {
	return eventually(
		() =>
			Array.from(
				document.querySelectorAll<HTMLElement>(`[data-testid="${testId}"]`),
			).find(el => el.checkVisibility({ visibilityProperty: true })),
		`the test id ${testId}`,
	);
}

/** The first element matching `selector` once the page has one, seen or
 * not: for what has no test id, or is never shown, like a file input. */
export function present<E extends Element = HTMLElement>(
	selector: string,
): Promise<E> {
	return eventually(() => document.querySelector<E>(selector), selector);
}

/** Click the middle of the element: on the innermost thing there, which is
 * where a handler inside it expects the click to land, and not at all when
 * something else covers it. Only a click: what reacts to the pointer going
 * down or up gets nothing. */
export async function tap(testId: string): Promise<void> {
	const tapped = await find(testId);
	tapped.scrollIntoView({ block: 'nearest' });
	const box = tapped.getBoundingClientRect();
	const at = {
		clientX: box.x + box.width / 2,
		clientY: box.y + box.height / 2,
	};
	const touched = document.elementFromPoint(at.clientX, at.clientY);
	if (touched === null || !tapped.contains(touched)) {
		throw new Error(`${testId} is covered: a tap on it lands elsewhere`);
	}
	// Not `click()`: what is touched may be a drawing inside the element.
	touched.dispatchEvent(
		new MouseEvent('click', {
			...at,
			bubbles: true,
			cancelable: true,
			composed: true,
		}),
	);
	await settled();
}

/** Replace what the field with `testId`, or the one inside it, holds. */
export async function fill(testId: string, text: string): Promise<void> {
	const named = await find(testId);
	const field =
		named instanceof HTMLInputElement || named instanceof HTMLTextAreaElement
			? named
			: named.querySelector<HTMLInputElement | HTMLTextAreaElement>(
					'input, textarea',
				);
	if (field === null) throw new Error(`${testId} holds no field to fill`);
	field.focus();
	field.value = text;
	field.dispatchEvent(new InputEvent('input', { bubbles: true }));
	await settled();
}
