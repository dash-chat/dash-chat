import type { Agent } from '../../setup/setup-agents';

export interface BackPopsSteps {
	/** Click from the parent page into the page under test. */
	open: () => Promise<void>;
	/** Wait for the page under test. */
	ready: () => Promise<void>;
	/** Click the page's Back control. */
	back: () => Promise<void>;
	/** Wait for the parent page. */
	parentReady: () => Promise<void>;
}

/**
 * Enter a page, press its Back, and prove the press popped the history stack
 * rather than pushing onto it: after a pop, `history.forward()` returns to
 * the page, while a `goto`-wired Back leaves nothing ahead to go forward to.
 * Presses Back once more afterwards, ending where it started, on the parent.
 */
export async function expectBackPops(
	agent: Agent,
	steps: BackPopsSteps,
): Promise<void> {
	await steps.open();
	await steps.ready();
	const pagePath = await agent.execute(() => window.__test.currentPath());

	await steps.back();
	await steps.parentReady();
	const parentPath = await agent.execute(() => window.__test.currentPath());
	expect(parentPath).not.toEqual(pagePath);

	await agent.execute(() => window.__test.historyForward());
	await agent.waitUntil(
		async () =>
			(await agent.execute(() => window.__test.currentPath())) === pagePath,
		{
			timeoutMsg: `Back on ${pagePath} pushed instead of popping — history.forward() did not return to it`,
		},
	);
	await steps.ready();

	await steps.back();
	await steps.parentReady();
}
