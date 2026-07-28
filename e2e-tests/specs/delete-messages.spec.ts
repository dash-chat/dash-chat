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

		await mine.delete();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(await agent2.tr('thisMessageWasDeleted'));
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

		await mine.delete();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(await agent2.tr('thisMessageWasDeleted'));
	});

	it('does not offer Delete on the peer’s messages', async () => {
		await agent2.directChatPage.composer.sendMessage("Bob's message stays");
		const message = await agent1.directChatPage.messages.waitForMessage(
			"Bob's message stays",
		);

		await message.openActions();
		expect(await message.deleteAction.isExisting()).toBe(false);
	});
});
