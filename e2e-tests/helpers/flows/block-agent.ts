import type { Agent } from '../../setup/setup-agents';

/**
 * Block the peer of the currently open direct chat from its chat-settings
 * page, leaving the agent back on the direct-chat page.
 */
export async function blockAgent(agent: Agent): Promise<void> {
	await agent.directChatPage.settingsLink.click();
	await agent.chatSettingsPage.ready();

	await agent.chatSettingsPage.blockButton.click();
	await agent.chatSettingsPage.blockConfirm.waitForClickable();
	await agent.chatSettingsPage.blockConfirm.click();
	await agent.chatSettingsPage.blockConfirm.waitForClickable({
		reverse: true,
	});

	await agent.chatSettingsPage.back.click();
	await agent.directChatPage.ready();
}
