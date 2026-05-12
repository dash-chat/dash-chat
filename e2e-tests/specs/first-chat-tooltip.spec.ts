/**
 * FirstChatTooltip E2E test.
 *
 * Verifies the "Start your first chat here" tooltip appears on first run
 * when the chat list is empty, dismisses on click, and never reappears.
 *
 * Only needs one agent — uses agent1.
 */

import { S } from '../../ui/tests/selectors';
import { type Agent, makeAgent, waitForTestUtils } from '../helpers/setup-agents';

describe('FirstChatTooltip', () => {
	let agent: Agent;

	before(async () => {
		agent = makeAgent(browser.getInstance('agent1'));
		await waitForTestUtils(agent);

		// Force narrow layout so the tooltip renders (default window is 800px, above the 768px threshold)
		await agent.execute(() =>
			window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false })),
		);

		// Clear any previous tooltip state so the test starts fresh
		await agent.execute(() => localStorage.removeItem('first-chat-tooltip-shown'));
		await agent.createProfile('Tooltip', 'Test');
	});

	it('shows tooltip on first run when chat list is empty', async () => {
		await agent.waitUntil(async () => !!(await agent.homeLoaded()), {
			timeout: 10_000,
			timeoutMsg: 'Home page empty state not visible',
		});
		await agent.waitUntil(async () => !!(await agent.firstChatTooltip()), {
			timeout: 5_000,
			timeoutMsg: 'First chat tooltip not visible on first run',
		});
	});

	it('dismisses tooltip on click', async () => {
		await agent.click(S.home.firstChatTooltip);

		await agent.waitUntil(async () => !(await agent.firstChatTooltip()), {
			timeout: 5_000,
			timeoutMsg: 'Tooltip was not dismissed after click',
		});
	});

	it('does not reappear after page reload', async () => {
		await agent.execute(() => {
			window.location.href = '/';
		});

		await waitForTestUtils(agent);
		await agent.waitUntil(async () => !!(await agent.homeLoaded()), {
			timeout: 10_000,
			timeoutMsg: 'Home page not loaded after reload',
		});

		expect(await agent.firstChatTooltip()).toBeFalsy();
	});
});
