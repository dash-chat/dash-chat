/**
 * The two-panel desktop layout has no bottom tab bar: a vertical strip beside
 * the sidebar carries the same tabs and switches the sidebar's panel.
 */
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Desktop nav strip', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'desktop' }]);
		await agent.createProfilePage.createProfile('Alice', 'Strip');
		await agent.homePage.ready();
		if (!(await agent.supportsWideScreen())) this.skip();
		await agent.setWideScreen(true);
	});

	it('switches the sidebar between settings and chats', async () => {
		await agent.mainTabbar.desktopNavStrip.waitForDisplayed();
		await agent.homePage.emptyState.waitForDisplayed();

		await agent.mainTabbar.settingsTab.click();
		await agent.settingsPage.ready();
		await agent.homePage.emptyState.waitForExist({ reverse: true });

		await agent.mainTabbar.chatsTab.click();
		await agent.homePage.ready();
	});
});
