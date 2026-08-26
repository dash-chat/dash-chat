import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Delete account', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		await agent.createProfilePage.createProfile('Delete', 'Account');
	});

	it('wipes all data and restarts at first launch when confirmed', async () => {
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.accountLink.click();
		await agent.accountPage.ready();

		await agent.accountPage.deleteItem.click();
		await agent.accountPage.deleteConfirm.click();

		await agent.waitForAppExit();
		if (agent.platform === 'desktop') {
			// Desktop delete_account restarts the app into a process the
			// WebDriver session can't reattach to; stop it and let startApp
			// launch a driveable one on the same data dir.
			await agent.stopApp();
		}
		await agent.startApp();

		await agent.welcomePage.ready();
		await agent.createProfilePage.createProfile('Fresh', 'Start');
	});
});
