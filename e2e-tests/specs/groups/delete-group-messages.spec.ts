import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Deleting group messages', () => {
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

	it('deletes a message for everyone, showing a placeholder on both sides', async () => {
		await agent1.groupChatPage.composer.sendMessage('Delete me');
		const mine =
			await agent1.groupChatPage.messages.waitForMessage('Delete me');
		const theirs =
			await agent2.groupChatPage.messages.waitForMessage('Delete me');

		await mine.delete();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(
			await agent2.tr('someoneDeletedThisMessage', { name: 'Alice Test' }),
		);
	});

	it('does not offer Delete on another member’s messages', async () => {
		await agent2.groupChatPage.composer.sendMessage("Bob's message stays");
		const message = await agent1.groupChatPage.messages.waitForMessage(
			"Bob's message stays",
		);

		await message.openActions();
		expect(await message.deleteAction.isExisting()).toBe(false);
	});
});
