import type { Agent } from '../../setup/setup-agents';

/**
 * Delete the agent's account, walking from the home page: settings → account
 * → delete → confirm, then relaunch the app into the first-launch welcome
 * screen. Follow with `createProfilePage.createProfile(...)` to start over as
 * a fresh identity.
 */
export async function deleteAccount(agent: Agent): Promise<void> {
	await agent.homePage.settingsLink.click();
	await agent.settingsPage.ready();
	await agent.settingsPage.accountLink.click();
	await agent.accountPage.ready();
	await agent.accountPage.deleteItem.click();
	await agent.accountPage.deleteConfirm.click();
	await agent.waitForAppExit();
	if (agent.platform === 'desktop') {
		// Desktop delete_account restarts the app into a process the WebDriver
		// session can't reattach to; stop it and let startApp launch a
		// driveable one on the same data dir.
		await agent.stopApp();
	}
	await agent.startApp();
	await agent.welcomePage.ready();
}
