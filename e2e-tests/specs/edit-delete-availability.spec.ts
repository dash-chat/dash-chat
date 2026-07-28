import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

// Smoke test for the edit/delete action affordances in an established direct
// chat: both actions must be offered on your own messages and withheld on the
// peer's. Guards against the quick-action props being wired into only one of the
// direct-chat page's rendering branches.
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
		const message =
			await agent1.directChatPage.messages.waitForMessage('My own message');

		await message.openActions();
		await message.editAction.waitForExist();

		expect(await message.editAction.isExisting()).toBe(true);
		expect(await message.deleteAction.isExisting()).toBe(true);
	});

	it('offers neither Edit nor Delete on the peer’s message', async () => {
		await agent2.directChatPage.composer.sendMessage("Peer's message");
		const message =
			await agent1.directChatPage.messages.waitForMessage("Peer's message");

		await message.openActions();
		expect(await message.editAction.isExisting()).toBe(false);
		expect(await message.deleteAction.isExisting()).toBe(false);
	});
});
