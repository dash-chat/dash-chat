import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

// Smoke test for the edit/delete action affordances in a group chat: both
// actions must be offered on your own messages and withheld on another member's.
describe('Edit/delete action availability (group chat)', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
		await agent1.groupChatPage.ready();

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('offers both Edit and Delete on your own message', async () => {
		await agent1.groupChatPage.composer.sendMessage('My own message');
		const message =
			await agent1.groupChatPage.messages.waitForMessage('My own message');

		await message.openActions();
		await message.editAction.waitForExist();

		expect(await message.editAction.isExisting()).toBe(true);
		expect(await message.deleteAction.isExisting()).toBe(true);
	});

	it('offers neither Edit nor Delete on another member’s message', async () => {
		await agent2.groupChatPage.composer.sendMessage("Bob's message");
		const message =
			await agent1.groupChatPage.messages.waitForMessage("Bob's message");

		await message.openActions();
		expect(await message.editAction.isExisting()).toBe(false);
		expect(await message.deleteAction.isExisting()).toBe(false);
	});
});
