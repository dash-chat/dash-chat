/**
 * FirstChatTooltip E2E test.
 *
 * Verifies the "Start your first chat here" tooltip appears on first run
 * when the chat list is empty, dismisses on click, and never reappears.
 *
 * Only needs one agent — uses agent1.
 */

import {
	type Agent,
	setupAgent,
	waitForTestUtils,
} from '../setup/setup-agents';

describe('FirstChatTooltip', () => {
	let agent: Agent;

	before(async () => {
		agent = await setupAgent('agent1');

		await agent.execute(() =>
			localStorage.removeItem('first-chat-tooltip-shown'),
		);
		await agent.createProfilePage.createProfile('Tooltip', 'Test');
	});

	it('shows tooltip on first run when chat list is empty', async () => {
		await agent.homePage.ready();
		await agent.homePage.firstChatTooltip.waitForExist();
	});

	it('dismisses tooltip on click', async () => {
		await agent.homePage.firstChatTooltip.click();
		await agent.homePage.firstChatTooltip.waitForExist({ reverse: true });
	});

	it('does not reappear after page reload', async () => {
		await agent.execute(() => {
			window.location.href = '/';
		});

		await waitForTestUtils(agent);
		await agent.homePage.ready();

		expect(await agent.homePage.firstChatTooltip.isExisting()).toBe(false);
	});
});
