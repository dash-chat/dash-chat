import type { Agent } from '../../setup/setup-agents';

/** Walk from the agent's chat list through the seven taps on the version row
 *  that unlock the developer settings, and open them. */
export async function unlockDeveloperSettings(agent: Agent): Promise<void> {
	await agent.mainTabbar.settingsTab.click();
	await agent.settingsPage.ready();
	await agent.settingsPage.helpLink.click();
	await agent.helpPage.ready();
	await agent.helpPage.tapVersion(7);
	await agent.helpPage.back.click();
	await agent.settingsPage.ready();
	await agent.settingsPage.developerLink.click();
	await agent.developerPage.ready();
}

/** Walk from the agent's chat list, or its open direct chat, to the unlocked
 *  developer settings. */
export async function openDeveloperSettings(agent: Agent): Promise<void> {
	if (await agent.directChatPage.back.isExisting()) {
		await agent.directChatPage.back.click();
	}
	await agent.homePage.ready();
	await agent.mainTabbar.settingsTab.click();
	await agent.settingsPage.ready();
	await agent.settingsPage.developerLink.click();
	await agent.developerPage.ready();
}

/** Walk from the developer settings back to the chat list. */
export async function returnToChatList(agent: Agent): Promise<void> {
	await agent.developerPage.back.click();
	await agent.settingsPage.ready();
	await agent.mainTabbar.chatsTab.click();
	await agent.homePage.ready();
}
