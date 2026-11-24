import type { Agent } from '../../setup/setup-agents';

/** Get `agent` back to its chat list if a chat is on screen. */
async function leaveChat(agent: Agent): Promise<void> {
	if (!(await agent.directChatPage.page.isExisting())) return;
	await agent.directChatPage.back.click();
	await agent.homePage.ready();
}

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
 * Make every one of `agents` a contact of every other, a pair at a time: each
 * of a pair adds the other's link and ends up on their direct chat. Two agents
 * is one exchange; more is every pair of them.
 */
export async function exchangeContacts(agents: Agent[]): Promise<void> {
	for (let i = 0; i < agents.length; i++) {
		for (let j = i + 1; j < agents.length; j++) {
			await exchangePair(agents[i], agents[j]);
		}
	}
}

async function exchangePair(agent1: Agent, agent2: Agent): Promise<void> {
	const [link1, link2] = await Promise.all(
		[agent1, agent2].map(async agent => {
			// An exchange ends on the pair's direct chat, and the next one
			// starts from the chat list: with three agents or more, the second
			// pair begins where the first left off.
			await leaveChat(agent);
			await navigateToAddContact(agent);
			return await agent.addContactPage.getAddContactLink();
		}),
	);
	await agent1.addContactPage.enterAddContactLink(link2);
	await agent1.directChatPage.ready();
	await agent2.addContactPage.enterAddContactLink(link1);
	await agent2.directChatPage.ready();
}
