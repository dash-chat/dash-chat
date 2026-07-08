import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Deleting messages', () => {
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

	it('deletes a message for everyone, showing a placeholder on both sides', async () => {
		await agent1.directChatPage.sendMessage('Delete me');
		await agent1.directChatPage.messages.waitForMessage('Delete me');
		await agent2.directChatPage.messages.waitForMessage('Delete me');

		await agent1.directChatPage.messages.deleteMessage('Delete me');

		await agent1.directChatPage.messages.waitForDeleted(
			'Delete me',
			'You deleted this message.',
		);
		await agent2.directChatPage.messages.waitForDeleted(
			'Delete me',
			'This message was deleted.',
		);
	});

	it('deletes an edited message via its latest edit', async () => {
		await agent1.directChatPage.sendMessage('Draft v1');
		await agent1.directChatPage.messages.waitForMessage('Draft v1');
		await agent1.directChatPage.messages.editMessage('Draft v1', 'Draft v2');
		await agent1.directChatPage.messages.waitForMessage('Draft v2');
		await agent2.directChatPage.messages.waitForMessage('Draft v2');

		await agent1.directChatPage.messages.deleteMessage('Draft v2');

		await agent1.directChatPage.messages.waitForDeleted(
			'Draft v2',
			'You deleted this message.',
		);
		await agent2.directChatPage.messages.waitForDeleted(
			'Draft v2',
			'This message was deleted.',
		);
	});

	it('does not offer Delete on the peer’s messages', async () => {
		await agent2.directChatPage.sendMessage("Bob's message stays");
		await agent1.directChatPage.messages.waitForMessage("Bob's message stays");

		await agent1.directChatPage.messages.openActions("Bob's message stays");
		expect(
			await agent1.directChatPage.messages.quickDeleteButton.isExisting(),
		).toBe(false);
	});
});
