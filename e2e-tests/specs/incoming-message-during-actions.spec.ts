import { createProfilesAndExchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Receiving a message while the actions menu is open', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfilesAndExchangeContacts({ Alice: agent1, Bob: agent2 });
	});

	it('leaves the menu open and still acting on its own message', async () => {
		await agent1.directChatPage.composer.sendMessage('Anchor');
		const anchor =
			await agent2.directChatPage.messages.waitForMessage('Anchor');

		await anchor.openActions();

		await agent1.directChatPage.composer.sendMessage('Arrived meanwhile');
		await agent2.directChatPage.messages.waitForMessage('Arrived meanwhile');

		await anchor.expectActionsMenuToStayOpen();

		// Deleting through the still-open menu must hit the anchor message, not
		// the one that arrived under it.
		await anchor.deleteAction.click();
		await anchor.deleteDialog.waitForDisplayed();
		await anchor.deleteForMeDialogConfirm.click();

		await agent2.directChatPage.messages.waitForMessageGone('Anchor');
		expect(
			await agent2.directChatPage.messages.messageAreaContains(
				'Arrived meanwhile',
			),
		).toBe(true);
	});
});
