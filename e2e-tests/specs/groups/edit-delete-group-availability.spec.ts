import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../../setup/setup-agents';

// Smoke test for the edit/delete action affordances in a group chat: both
// actions must be offered on your own messages and withheld on another member's.
describe('Edit/delete action availability (group chat)', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
		await agent1.groupChatPage.ready();

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('offers both Edit and Delete on your own message', async () => {
		await agent1.groupChatPage.sendMessage('My own message');
		await agent1.groupChatPage.messages.waitForMessage('My own message');

		await agent1.groupChatPage.messages.openActions('My own message');
		await agent1.groupChatPage.messages.quickEditButton.waitForExist();

		expect(
			await agent1.groupChatPage.messages.quickEditButton.isExisting(),
		).toBe(true);
		expect(
			await agent1.groupChatPage.messages.quickDeleteButton.isExisting(),
		).toBe(true);
	});

	it('offers neither Edit nor Delete on another member’s message', async () => {
		await agent2.groupChatPage.sendMessage("Bob's message");
		await agent1.groupChatPage.messages.waitForMessage("Bob's message");

		await agent1.groupChatPage.messages.openActions("Bob's message");
		expect(
			await agent1.groupChatPage.messages.quickEditButton.isExisting(),
		).toBe(false);
		expect(
			await agent1.groupChatPage.messages.quickDeleteButton.isExisting(),
		).toBe(false);
	});
});
