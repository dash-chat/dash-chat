/**
 * FirstChatTooltip E2E test.
 *
 * Verifies the "Start your first chat here" tooltip appears on first run
 * when the chat list is empty, dismisses on click, and never reappears.
 *
 * Only needs one agent — uses agent1.
 */

import { waitForTestUtils, createProfile } from '../helpers/setup-agents';

describe('FirstChatTooltip', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);

		// Clear any previous tooltip state so the test starts fresh
		await agent.execute(() => localStorage.removeItem('first-chat-tooltip-shown'));
		await createProfile(agent, 'Tooltip', 'Test');
	});

	it('shows tooltip on first run when chat list is empty', async () => {
		// Wait for home page with empty state
		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.homeLoaded() !== null),
			{ timeout: 10_000, timeoutMsg: 'Home page empty state not visible' },
		);

		// Tooltip should be visible
		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.firstChatTooltip() !== null),
			{ timeout: 5_000, timeoutMsg: 'First chat tooltip not visible on first run' },
		);
	});

	it('dismisses tooltip on click', async () => {
		// Click the tooltip
		await agent.execute(() => {
			(window.__test.firstChatTooltip() as HTMLElement)?.click();
		});

		// Verify tooltip is removed from DOM
		await agent.waitUntil(
			async () => {
				const exists = await agent.execute(
					() => window.__test.firstChatTooltip() !== null,
				);
				return !exists;
			},
			{ timeout: 5_000, timeoutMsg: 'Tooltip was not dismissed after click' },
		);
	});

	it('does not reappear after page reload', async () => {
		// Reload the page
		await agent.execute(() => {
			window.location.href = '/';
		});

		// Wait for test utils and home page to load
		await waitForTestUtils(agent);
		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.homeLoaded() !== null),
			{ timeout: 10_000, timeoutMsg: 'Home page not loaded after reload' },
		);

		// Tooltip should NOT be visible
		const exists = await agent.execute(
			() => window.__test.firstChatTooltip() !== null,
		);
		expect(exists).toBe(false);
	});
});
