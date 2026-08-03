import type { ChainablePromiseElement } from 'webdriverio';

/** Comfortably past the 500ms threshold in the app's `longpress` action. */
const LONG_PRESS_MS = 700;

/**
 * Dispatch one touch event at the centre of `element`. Serialized into the
 * page by `execute`, so it has to stay self-contained — and `execute` widens
 * its arguments, so the gesture is narrowed by the caller rather than here.
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
	await agent.execute(dispatchTouch, element, 'touchstart');
	await agent.pause(LONG_PRESS_MS);
	await agent.execute(dispatchTouch, element, 'touchend');
}
