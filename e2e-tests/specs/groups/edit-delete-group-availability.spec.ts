import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

// Smoke test for the edit/delete action affordances in a group chat: Edit is
// offered only on your own messages, while Delete is offered on every message
// (delete-for-me is always available; delete-for-everyone is gated inside the
// confirmation dialog).
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
		await agent1.groupChatPage.messages.waitForMessage('My own message');

		await agent1.groupChatPage.messages.openMessageActions('My own message');
		const editAction =
			await agent1.groupChatPage.messages.editAction('My own message');
		await editAction.waitForExist();
		expect(await editAction.isExisting()).toBe(true);
		expect(
			await (
				await agent1.groupChatPage.messages.deleteAction('My own message')
			).isExisting(),
		).toBe(true);
	});

	it('offers Delete but not Edit on another member’s message', async () => {
		await agent2.groupChatPage.composer.sendMessage("Bob's message");
		await agent1.groupChatPage.messages.waitForMessage("Bob's message");

		await agent1.groupChatPage.messages.openMessageActions("Bob's message");
		expect(
			await (
				await agent1.groupChatPage.messages.deleteAction("Bob's message")
			).isExisting(),
		).toBe(true);
		expect(
			await (
				await agent1.groupChatPage.messages.editAction("Bob's message")
			).isExisting(),
		).toBe(false);
	});
});
