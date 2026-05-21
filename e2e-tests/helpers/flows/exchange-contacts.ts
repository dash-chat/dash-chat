import { tid } from '../../../ui/tests/selectors';
import type { Agent } from '../setup-agents';

async function navigateToAddContact(agent: Agent): Promise<void> {
	await agent.homePage.newMessageButton.click();
	await agent.newMessagePage.addContact.click();
}

export async function exchangeContacts(
	agent1: Agent,
	agent2: Agent,
): Promise<void> {
	await navigateToAddContact(agent1);
	await navigateToAddContact(agent2);
	const code1 = await agent1.addContactPage.getContactCode();
	const code2 = await agent2.addContactPage.getContactCode();
	if (!code1) throw new Error('agent1 contact code missing');
	if (!code2) throw new Error('agent2 contact code missing');
	await agent1.addContactPage.enterCode(code2);
	await agent1.$(tid('direct-chat-page')).waitForExist();
	await agent2.addContactPage.enterCode(code1);
	await agent2.$(tid('direct-chat-page')).waitForExist();
}
