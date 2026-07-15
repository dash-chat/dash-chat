import type { Agent } from '../../setup/setup-agents';
import { SYNC_TIMEOUT } from '../timeouts';

/** Walk from the home screen to the add-contact page. */
export async function navigateToAddContact(agent: Agent): Promise<void> {
	await agent.homePage.newMessageButton.click();
	await agent.newMessagePage.addContact.click();
	await agent.addContactPage.ready();
}

/**
 * Two-way contact exchange: both agents add each other's code and end up on
 * their respective direct-chat pages.
 */
export async function exchangeContacts(
	agent1: Agent,
	agent2: Agent,
): Promise<void> {
	await navigateToAddContact(agent1);
	await navigateToAddContact(agent2);
	const code1 = await agent1.addContactPage.getContactCode();
	const code2 = await agent2.addContactPage.getContactCode();
	// Adding a contact bootstraps the chat's spaces and topics in the node
	// before navigating to the chat — slower than the UI timeout on a phone.
	await agent1.addContactPage.enterCode(code2);
	await agent1.directChatPage.ready(SYNC_TIMEOUT);
	await agent2.addContactPage.enterCode(code1);
	await agent2.directChatPage.ready(SYNC_TIMEOUT);
}
