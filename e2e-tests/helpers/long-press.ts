import type { ChainablePromiseElement } from 'webdriverio';

/** Comfortably past the 500ms threshold in the app's `longpress` action. */
const LONG_PRESS_MS = 700;

/**
 * Dispatch one touch event at the centre of `element`. Serialized into the
 * page by `execute`, so it has to stay self-contained.
 */
function dispatchTouch(element: HTMLElement, gesture: string) {
	const rect = element.getBoundingClientRect();
	const clientX = rect.left + rect.width / 2;
	const clientY = rect.top + rect.height / 2;
	const touch = new Touch({ identifier: 1, target: element, clientX, clientY });
	const down = gesture === 'touchstart';
	element.dispatchEvent(
		new TouchEvent(gesture, {
			bubbles: true,
			cancelable: true,
			touches: down ? [touch] : [],
			targetTouches: down ? [touch] : [],
			changedTouches: [touch],
		}),
	);
}

/**
 * Hold a touch on `element` past the app's long-press threshold, then lift it
 * — the gesture a mobile user makes to open a context menu. The app starts its
 * own timer off `touchstart`, so the hold has to happen between the two events.
 */
export async function simulateLongpress(
	agent: WebdriverIO.Browser,
	element: ChainablePromiseElement,
): Promise<void> {
	// `execute` only turns an argument into a page node when it is a resolved
	// element; the chainable promise serializes to `{}` instead.
	const target = await element.getElement();
	await agent.execute(dispatchTouch, target, 'touchstart');
	await agent.pause(LONG_PRESS_MS);
	await agent.execute(dispatchTouch, target, 'touchend');
}
