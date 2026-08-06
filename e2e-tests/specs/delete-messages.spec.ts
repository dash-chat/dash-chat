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
		await agent1.directChatPage.composer.sendMessage('Delete me');
		const mine =
			await agent1.directChatPage.messages.waitForMessage('Delete me');
		const theirs =
			await agent2.directChatPage.messages.waitForMessage('Delete me');

		await mine.deleteForEveryone();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(
			await agent2.tr('someoneDeletedThisMessage', { name: 'Alice Test' }),
		);
	});

	it('deletes an edited message via its latest edit', async () => {
		await agent1.directChatPage.composer.sendMessage('Draft v1');
		const mine =
			await agent1.directChatPage.messages.waitForMessage('Draft v1');
		await agent2.directChatPage.messages.waitForMessage('Draft v1');

		await mine.edit('Draft v1', 'Draft v2');
		await agent1.directChatPage.messages.waitForMessage('Draft v2');
		const theirs =
			await agent2.directChatPage.messages.waitForMessage('Draft v2');

		await mine.deleteForEveryone();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(
			await agent2.tr('someoneDeletedThisMessage', { name: 'Alice Test' }),
		);
	});

	it('deletes a message only for me, leaving no placeholder', async () => {
		await agent1.directChatPage.composer.sendMessage('Just for me');
		const message =
			await agent1.directChatPage.messages.waitForMessage('Just for me');
		await agent2.directChatPage.messages.waitForMessage('Just for me');

		await message.deleteForMe();

		// Gone on my side (no placeholder, unlike delete-for-everyone)...
		await agent1.directChatPage.messages.waitForMessageGone('Just for me');
		// ...but still visible for the peer.
		expect(
			await agent2.directChatPage.messages.messageAreaContains('Just for me'),
		).toBe(true);
	});

	it('offers Delete for me (but not Delete for everyone) on the peer’s messages', async () => {
		await agent2.directChatPage.composer.sendMessage("Bob's message");
		const message =
			await agent1.directChatPage.messages.waitForMessage("Bob's message");

		await message.openDeleteDialog();

		// Only "Delete for me" is available for a received message.
		await message.deleteForMeDialogConfirm.waitForExist();
		expect(await message.deleteForEveryoneDialogConfirm.isExisting()).toBe(false);

		await message.deleteForMeDialogConfirm.click();
		await agent1.directChatPage.messages.waitForMessageGone("Bob's message");
		expect(
			await agent2.directChatPage.messages.messageAreaContains("Bob's message"),
		).toBe(true);
	});
});
