import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

// Smoke test for the edit/delete action affordances in an established direct
// chat: Edit is offered only on your own messages, while Delete is offered on
// every message (delete-for-me is always available; delete-for-everyone is
// gated inside the confirmation dialog). Guards against the action props being
// wired into only one of the direct-chat page's rendering branches.
describe('Edit/delete action availability (direct chat)', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('offers both Edit and Delete on your own message', async () => {
		await agent1.directChatPage.composer.sendMessage('My own message');
		await agent1.directChatPage.messages.waitForMessage('My own message');

		await agent1.directChatPage.messages.openMessageActions('My own message');
		const editAction =
			await agent1.directChatPage.messages.editAction('My own message');
		await editAction.waitForExist();
		expect(await editAction.isExisting()).toBe(true);
		expect(
			await (
				await agent1.directChatPage.messages.deleteAction('My own message')
			).isExisting(),
		).toBe(true);
	});

	it('offers Delete but not Edit on the peer’s message', async () => {
		await agent2.directChatPage.composer.sendMessage("Peer's message");
		await agent1.directChatPage.messages.waitForMessage("Peer's message");

		await agent1.directChatPage.messages.openMessageActions("Peer's message");
		expect(
			await (
				await agent1.directChatPage.messages.deleteAction("Peer's message")
			).isExisting(),
		).toBe(true);
		expect(
			await (
				await agent1.directChatPage.messages.editAction("Peer's message")
			).isExisting(),
		).toBe(false);
	});
});
