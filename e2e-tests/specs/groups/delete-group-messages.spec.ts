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

		await mine.deleteForEveryone();

		await mine.waitForDeleted(await agent1.tr('youDeletedThisMessage'));
		await theirs.waitForDeleted(
			await agent2.tr('someoneDeletedThisMessage', { name: 'Alice Test' }),
		);
	});

	// The delete-for-me op lives in the deleter's private device-group topic and
	// tombstones the target in the shared chat topic, so the message must vanish
	// for the deleter while staying visible to every other member.
	it('deletes my own message only for me, leaving other members untouched', async () => {
		await agent1.groupChatPage.composer.sendMessage('Only I forget this');
		const message =
			await agent1.groupChatPage.messages.waitForMessage('Only I forget this');
		await agent2.groupChatPage.messages.waitForMessage('Only I forget this');

		await message.deleteForMe();

		// Gone on my side with no placeholder (unlike delete-for-everyone)...
		await agent1.groupChatPage.messages.waitForMessageGone(
			'Only I forget this',
		);
		// ...but still visible to the other member.
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				'Only I forget this',
			),
		).toBe(true);
	});

	it("deletes another member's message only for me", async () => {
		await agent2.groupChatPage.composer.sendMessage("Bob's group message");
		const message = await agent1.groupChatPage.messages.waitForMessage(
			"Bob's group message",
		);

		await message.openDeleteDialog();

		// Only "Delete for me" is available for another member's message.
		await message.deleteForMeDialogConfirm.waitForExist();
		expect(await message.deleteForEveryoneDialogConfirm.isExisting()).toBe(
			false,
		);

		await message.deleteForMeDialogConfirm.click();

		await agent1.groupChatPage.messages.waitForMessageGone(
			"Bob's group message",
		);
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				"Bob's group message",
			),
		).toBe(true);
	});
});
