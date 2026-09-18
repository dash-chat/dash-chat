import type { Agent } from '../../setup/setup-agents';
import { createProfiles } from './create-profiles';

/** Walk from the home screen to the add-contact page. */
export async function navigateToAddContact(agent: Agent): Promise<void> {
	await agent.homePage.newMessageButton.click();
	await agent.newMessagePage.ready();
	await agent.newMessagePage.addContact.click();
	await agent.addContactPage.ready();
}

/** An agent's own add-contact link, leaving it back on its chat list. */
export async function contactLinkOf(agent: Agent): Promise<string> {
	await navigateToAddContact(agent);
	const link = await agent.addContactPage.getAddContactLink();
	await agent.addContactPage.back.click();
	await agent.newMessagePage.back.click();
	await agent.homePage.ready();
	return link;
}

/**
 * Two-way contact exchange: both agents add each other's link and end up on
 * their respective direct-chat pages.
 */
export async function exchangeContacts(agents: [Agent, Agent]): Promise<void> {
	const [agent1, agent2] = agents;
	const [link1, link2] = await Promise.all(
		agents.map(async agent => {
			await navigateToAddContact(agent);
			return await agent.addContactPage.getAddContactLink();
		}),
	);
	await agent1.addContactPage.enterAddContactLink(link2);
	await agent1.directChatPage.ready();
	await agent2.addContactPage.enterAddContactLink(link1);
	await agent2.directChatPage.ready();
}

/** Bootstrap two fresh agents, each named after its key, into contacts, each
 *  left on its direct chat with the other. */
export async function createProfilesAndExchangeContacts(
	profiles: Record<string, Agent>,
): Promise<void> {
	const agents = Object.values(profiles);
	if (agents.length !== 2) {
		throw new Error(`expected two profiles, got ${agents.length}`);
	}
	await createProfiles(profiles);
	await exchangeContacts([agents[0], agents[1]]);
}
