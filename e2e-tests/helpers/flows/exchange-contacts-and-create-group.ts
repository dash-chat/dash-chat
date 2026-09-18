import type { Agent } from '../../setup/setup-agents';
import { backToHome } from './back-to-home';
import { createProfiles } from './create-profiles';
import { exchangeContacts } from './exchange-contacts';

/**
 * Bootstraps fresh agents into a shared group chat owned by the first listed:
 * creates profiles named after their keys, exchanges contacts between the owner
 * and each other agent, then walks the owner through the new-group flow with
 * all of them added as members. Leaves the owner on the group-chat page and
 * the members on the home page.
 */
export async function exchangeContactsAndCreateGroup(
	profiles: Record<string, Agent>,
): Promise<void> {
	await createProfiles(profiles);
	const [[, owner], ...members] = Object.entries(profiles);
	for (const [, member] of members) {
		await exchangeContacts([owner, member]);
		await backToHome([owner, member]);
	}

	await createGroup(
		owner,
		'mygroup',
		members.map(([name]) => name),
	);
}

/** Walk `agent` from the home page through the new-group flow, picking each
 *  of `members` by contact name, and leave it on the new group's chat page. An
 *  empty `members` makes a members-less group: the cheapest page where the
 *  connection chip is mounted. */
export async function createGroup(
	agent: Agent,
	groupName: string,
	members: string[],
): Promise<void> {
	await agent.homePage.ready();
	await agent.homePage.newMessageButton.click();
	await agent.newMessagePage.ready();
	await agent.newMessagePage.newGroup.click();

	await agent.newGroupPage.addMembersStep.ready();
	for (const member of members) {
		await agent.newGroupPage.addMembersStep.addContactByName(member);
	}
	await agent.newGroupPage.addMembersStep.nextButton.click();

	await agent.newGroupPage.groupInfoStep.ready();
	await agent.newGroupPage.groupInfoStep.setName(groupName);
	await agent.newGroupPage.groupInfoStep.createButton.click();

	await agent.groupChatPage.ready();
}
