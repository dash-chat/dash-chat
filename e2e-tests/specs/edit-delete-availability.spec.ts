import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

// Smoke test for the edit/delete action affordances in an established direct
// chat: both actions must be offered on your own messages and withheld on the
// peer's. Guards against the quick-action props being wired into only one of the
// direct-chat page's rendering branches.
describe('Edit/delete action availability (direct chat)', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('offers both Edit and Delete on your own message', async () => {
		await agent1.directChatPage.sendMessage('My own message');
		await agent1.directChatPage.messages.waitForMessage('My own message');

		await agent1.directChatPage.messages.openActions('My own message');
		await agent1.directChatPage.messages.quickEditButton.waitForExist();

		expect(
			await agent1.directChatPage.messages.quickEditButton.isExisting(),
		).toBe(true);
		expect(
			await agent1.directChatPage.messages.quickDeleteButton.isExisting(),
		).toBe(true);
	});

	it('offers neither Edit nor Delete on the peer’s message', async () => {
		await agent2.directChatPage.sendMessage("Peer's message");
		await agent1.directChatPage.messages.waitForMessage("Peer's message");

		await agent1.directChatPage.messages.openActions("Peer's message");
		expect(
			await agent1.directChatPage.messages.quickEditButton.isExisting(),
		).toBe(false);
		expect(
			await agent1.directChatPage.messages.quickDeleteButton.isExisting(),
		).toBe(false);
	});
});
