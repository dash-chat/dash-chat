/** Comfortably past the 500ms threshold in the app's `longpress` action. */
const LONG_PRESS_MS = 700;

/**
 * Dispatch one touch event at the centre of the element matching `selector`.
 * Serialized into the page by `execute`, so it has to stay self-contained.
 */
function dispatchTouch(selector: string, gesture: string) {
	const element = document.querySelector<HTMLElement>(selector);
	if (!element) throw new Error(`No element matching ${selector}`);
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
 * Hold a touch on the element matching `selector` past the app's long-press
 * threshold, then lift it — the gesture a mobile user makes to open a context
 * menu. The app starts its own timer off `touchstart`, so the hold has to
 * happen between the two events. A selector rather than an element: the page
 * can re-render during the hold, and a resolved handle would go stale.
 */
export async function simulateLongpress(
	agent: WebdriverIO.Browser,
	selector: string,
): Promise<void> {
	await agent.execute(dispatchTouch, selector, 'touchstart');
	await agent.pause(LONG_PRESS_MS);
	await agent.execute(dispatchTouch, selector, 'touchend');
}
