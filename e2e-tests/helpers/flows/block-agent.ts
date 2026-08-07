import type { Agent } from '../../setup/setup-agents';

/**
 * Block the peer of the currently open direct chat from its chat-settings
 * page. Blocking drops the agent back on the chat list, so this leaves it on
 * the home page.
 */
export async function blockAgent(agent: Agent): Promise<void> {
	await agent.directChatPage.settingsLink.click();
	await agent.chatSettingsPage.ready();

	await agent.chatSettingsPage.blockButton.click();
	await agent.chatSettingsPage.blockConfirm.waitForClickable();
	await agent.chatSettingsPage.blockConfirm.click();

	await agent.homePage.ready();
}
