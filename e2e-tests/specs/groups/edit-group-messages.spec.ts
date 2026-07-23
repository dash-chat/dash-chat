import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Editing group messages', () => {
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

	it('edits a message in place and shows the "Edited" indicator on both sides', async () => {
		await agent1.groupChatPage.composer.sendMessage('Helo group');
		await agent1.groupChatPage.messages.waitForMessage('Helo group');
		await agent2.groupChatPage.messages.waitForMessage('Helo group');

		await agent1.groupChatPage.messages.editMessage(
			'Helo group',
			'Hello group',
		);

		await agent1.groupChatPage.messages.waitForMessage('Hello group');
		await agent2.groupChatPage.messages.waitForMessage('Hello group');

		await browser.waitUntil(
			() => agent1.groupChatPage.messages.hasEditedIndicator('Hello group'),
			{ timeoutMsg: 'No "Edited" indicator on the author side' },
		);
		await browser.waitUntil(
			() => agent2.groupChatPage.messages.hasEditedIndicator('Hello group'),
			{ timeoutMsg: 'No "Edited" indicator on the peer side' },
		);
	});

	it('does not offer Edit on another member’s messages', async () => {
		await agent2.groupChatPage.composer.sendMessage("Bob's message");
		await agent1.groupChatPage.messages.waitForMessage("Bob's message");

		await agent1.groupChatPage.messages.openMessageActions("Bob's message");
		const messages = agent1.groupChatPage.messages;
		await (await messages.actionsMenu("Bob's message")).waitForDisplayed();
		expect(
			await (await messages.editAction("Bob's message")).isExisting(),
		).toBe(false);
	});
});
