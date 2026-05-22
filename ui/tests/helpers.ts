/**
 * DOM helpers used by `ui/tests/review/visit-all-pages.ts` and registered on
 * `window.__test` for in-browser test orchestration.
 *
 * Generic UI interaction (typing, clicking, waiting) for E2E specs lives in
 * `e2e-tests/helpers/pages/*` and drives the app via WDIO `$()` selectors.
 */

export async function waitFor(
	selector: string,
	timeout = 15_000,
): Promise<Element> {
	await waitUntil(() => !!document.querySelector(selector), timeout);
	return document.querySelector(selector)!;
}

export function waitUntil(
	condition: () => boolean,
	timeout = 15_000,
): Promise<void> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => reject(`Timeout`), timeout);
		const check = () => {
			if (condition()) {
				clearTimeout(timer);
				resolve();
			} else {
				setTimeout(check, 50);
			}
		};
		check();
	});
}

export function click(selector: string): void {
	const el =
		document.querySelector(selector + ' a') ?? document.querySelector(selector);
	if (!el) throw new Error(`click: element not found for "${selector}"`);
	(el as HTMLElement).click();
}

/** Resolves with the message text of the next `app:toast` event. */
export function captureNextToastMessage(timeout = 5_000): Promise<string> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(
			() => reject(new Error('Timeout waiting for toast')),
			timeout,
		);
		window.addEventListener(
			'app:toast',
			e => {
				clearTimeout(timer);
				resolve((e as CustomEvent<{ message: string }>).detail.message);
			},
			{ once: true },
		);
	});
}
