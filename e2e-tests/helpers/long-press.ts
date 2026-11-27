/**
 * Hold a touch on the element matching `selector` past the app's long-press
 * threshold, then lift it — the gesture a mobile user makes to open a context
 * menu. A selector rather than an element: the page can re-render during the
 * hold, and the gesture fails when what it pressed is gone by the time it
 * lifts.
 */
export async function simulateLongpress(
	agent: WebdriverIO.Browser,
	selector: string,
): Promise<void> {
	// Both paths settle: left unsettled, a press on something that went away
	// surfaces as a script timeout naming nothing.
	const failure = await agent.executeAsync<string | null, [string]>(
		(sel, done) =>
			void window.__test.longPress(sel).then(
				() => done(null),
				err => done(String(err)),
			),
		selector,
	);
	if (failure !== null) throw new Error(failure);
}
