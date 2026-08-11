import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Receiving a group message while the actions menu is open', () => {
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

	it('leaves the menu open and still acting on its own message', async () => {
		await agent1.groupChatPage.composer.sendMessage('Anchor');
		const anchor = await agent2.groupChatPage.messages.waitForMessage('Anchor');

		await anchor.openActions();

		await agent1.groupChatPage.composer.sendMessage('Arrived meanwhile');
		await agent2.groupChatPage.messages.waitForMessage('Arrived meanwhile');

		await anchor.expectActionsMenuToStayOpen();

		// Deleting through the still-open menu must hit the anchor message, not
		// the one that arrived under it.
		await anchor.deleteAction.click();
		await anchor.deleteDialog.waitForDisplayed();
		await anchor.deleteForMeDialogConfirm.click();

		await agent2.groupChatPage.messages.waitForMessageGone('Anchor');
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				'Arrived meanwhile',
			),
		).toBe(true);
	});
});
