import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Deleting messages', () => {
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

	it('deletes a message only for me, leaving no placeholder', async () => {
		await agent1.directChatPage.sendMessage('Just for me');
		await agent1.directChatPage.messages.waitForMessage('Just for me');
		await agent2.directChatPage.messages.waitForMessage('Just for me');

		await agent1.directChatPage.messages.deleteMessageForMe('Just for me');

		// Gone on my side (no placeholder, unlike delete-for-everyone)...
		await agent1.directChatPage.messages.waitForMessageGone('Just for me');
		// ...but still visible for the peer.
		expect(
			await agent2.directChatPage.messages.messageAreaContains('Just for me'),
		).toBe(true);
	});

	it('offers Delete for me (but not Delete for everyone) on the peer’s messages', async () => {
		await agent2.directChatPage.sendMessage("Bob's message");
		await agent1.directChatPage.messages.waitForMessage("Bob's message");

		await agent1.directChatPage.messages.openDeleteDialog("Bob's message");

		// Only "Delete for me" is available for a received message.
		await agent1.directChatPage.messages.deleteForMeConfirmButton.waitForExist();
		expect(
			await agent1.directChatPage.messages.deleteForEveryoneConfirmButton.isExisting(),
		).toBe(false);

		await agent1.directChatPage.messages.deleteForMeConfirmButton.click();
		await agent1.directChatPage.messages.waitForMessageGone("Bob's message");
		expect(
			await agent2.directChatPage.messages.messageAreaContains("Bob's message"),
		).toBe(true);
	});
});
