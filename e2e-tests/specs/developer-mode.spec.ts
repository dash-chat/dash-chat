import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Developer mode', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		await agent.createProfilePage.createProfile('Developer', 'Test');
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
	});

	it('hides the developer settings until it is unlocked', async () => {
		await expect(agent.settingsPage.developerLink).not.toBeDisplayed();
	});

	it('stays locked when the taps are too far apart', async () => {
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();

		await agent.helpPage.tapVersion(4);
		await agent.pause(600);
		await agent.helpPage.tapVersion(4);

		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
		await expect(agent.settingsPage.developerLink).not.toBeDisplayed();
	});

	it('unlocks after seven rapid taps on the version', async () => {
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();

		await agent.helpPage.tapVersion(7);
		await agent.toast.expectMessage(await agent.tr('developerModeEnabled'));

		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
		await expect(agent.settingsPage.developerLink).toBeDisplayed();
	});

	it('opens the developer page and disables developer mode again', async () => {
		await agent.settingsPage.developerLink.click();
		await agent.developerPage.ready();

		await agent.developerPage.disable.click();
		await agent.settingsPage.ready();

		await expect(agent.settingsPage.developerLink).not.toBeDisplayed();
	});
});
