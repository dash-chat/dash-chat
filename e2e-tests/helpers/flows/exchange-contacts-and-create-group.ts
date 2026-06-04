import type { Agent } from '../../setup/setup-agents';
import { exchangeContacts } from './exchange-contacts';

/**
 * Bootstraps two fresh agents into a shared group chat owned by agent1:
 * creates profiles, exchanges contacts, then walks agent1 through the
 * new-group flow with agent2 added as a member. Leaves agent1 on the
 * group-chat page and agent2 on the home page.
 */
export async function exchangeContactsAndCreateGroup(
	agent1: Agent,
	agent2: Agent,
): Promise<void> {
	await agent1.enablePreviewFeatures();
	await agent2.enablePreviewFeatures();
	await agent1.createProfilePage.createProfile('Alice', 'Test');
	await agent2.createProfilePage.createProfile('Bob', 'Test');
	await exchangeContacts(agent1, agent2);
	await agent1.directChatPage.back.click();
	await agent2.directChatPage.back.click();
	await agent1.homePage.ready();
	await agent2.homePage.ready();

	await agent1.homePage.newMessageButton.click();
	await agent1.newMessagePage.ready();
	await agent1.newMessagePage.newGroup.click();

	await agent1.newGroupPage.addMembersStep.ready();
	await agent1.newGroupPage.addMembersStep.addContactByName('Bob');
	await agent1.newGroupPage.addMembersStep.nextButton.click();

	await agent1.newGroupPage.groupInfoStep.ready();
	await agent1.newGroupPage.groupInfoStep.createButton.click();

	await agent1.groupChatPage.ready();
}
