import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Editing group messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
		await agent1.groupChatPage.ready();

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('edits a message in place and shows the "Edited" indicator on both sides', async () => {
		await agent1.groupChatPage.sendMessage('Helo group');
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

	it('shows the full edit history, original first', async () => {
		await agent1.groupChatPage.messages.openEditHistory('Hello group');
		const versions = await agent1.groupChatPage.messages.editHistoryVersions();
		expect(versions).toEqual(['Hello group', 'Helo group']);
	});

	it('does not offer Edit on another member’s messages', async () => {
		await agent2.groupChatPage.sendMessage("Bob's message");
		await agent1.groupChatPage.messages.waitForMessage("Bob's message");

		await agent1.groupChatPage.messages.openActions("Bob's message");
		expect(
			await agent1.groupChatPage.messages.quickEditButton.isExisting(),
		).toBe(false);
	});
});
