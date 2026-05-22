/**
 * Shared DOM helpers for test automation via webview_execute_js.
 */

export function waitFor(selector: string, timeout = 15_000): Promise<Element> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(
			() => reject(`Timeout waiting for ${selector}`),
			timeout,
		);
		const check = () => {
			const el = document.querySelector(selector);
			if (el) {
				clearTimeout(timer);
				resolve(el);
			} else {
				setTimeout(check, 50);
			}
		};
		check();
	});
}

export function waitForText(
	selector: string,
	text: string,
	timeout = 15_000,
): Promise<true> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(
			() => reject(`Timeout waiting for "${text}" in ${selector}`),
			timeout,
		);
		const check = () => {
			if (document.querySelector(selector)?.textContent?.includes(text)) {
				clearTimeout(timer);
				resolve(true);
			} else {
				setTimeout(check, 100);
			}
		};
		check();
	});
}

export function typeInto(selector: string, value: string): void {
	const el = document.querySelector(selector) as
		| HTMLInputElement
		| HTMLTextAreaElement
		| null;
	if (!el) throw new Error(`typeInto: element not found for "${selector}"`);
	const isTextArea = el.tagName === 'TEXTAREA';
	const proto = isTextArea
		? HTMLTextAreaElement.prototype
		: HTMLInputElement.prototype;
	const setter = Object.getOwnPropertyDescriptor(proto, 'value')!.set!;
	setter.call(el, value);
	el.dispatchEvent(new Event('input', { bubbles: true }));
	el.dispatchEvent(new Event('change', { bubbles: true }));
}

export function click(selector: string): void {
	const el =
		document.querySelector(selector + ' a') ?? document.querySelector(selector);
	if (!el) throw new Error(`click: element not found for "${selector}"`);
	(el as HTMLElement).click();
}

/** Wait for framework reactivity to settle.
 *  Uses setTimeout instead of requestAnimationFrame because rAF may never
 *  fire in offscreen/headless WebKitGTK contexts (e.g. tauri-driver). */
export function nextTick(): Promise<void> {
	return new Promise(r => setTimeout(r, 50));
}

/** Returns a promise that resolves with the message text of the next app:toast event. */
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
