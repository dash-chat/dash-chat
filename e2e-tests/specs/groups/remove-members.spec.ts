import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Removing group members', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);

		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();
	});

	it('admin can remove a non-admin member', async () => {
		await createGroup(agent1, 'Test Group', 'Bob');

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();

		// Click on Bob's member item to open the bottom sheet
		await agent1.groupInfoPage.memberItem('Bob').click();

		// Click "Remove member" in the sheet
		await agent1.groupInfoPage.removeMemberButton.waitForExist();
		await agent1.groupInfoPage.removeMemberButton.click();

		// Confirm in the dialog
		await agent1.groupInfoPage.removeMemberConfirmButton.waitForExist();
		await agent1.groupInfoPage.removeMemberConfirmButton.click();

		// Bob should no longer appear in the members list
		await agent1.groupInfoPage
			.memberItem('Bob')
			.waitForExist({ reverse: true });

		// From bob's point of view, check that they have been removed and see the system message about it
		await agent2.homePage.ready();
		await agent2.homePage.chatListItem('Test Group').click();
		await agent2.groupChatPage.ready();

		const systemMessage = agent2.$(
			'[data-testid="group-chat-system-message-group_member_removed"]',
		);
		await expect(systemMessage).toBeExisting();
		const expectedText = await agent2.tr('someoneRemovedYouFromTheGroup', {
			name: 'Alice Test',
		});
		await expect(systemMessage).toHaveText(expectedText);

		// Message input is disabled (no longer a member)
		await expect(agent2.groupChatPage.messageInput).not.toBeEnabled();
	});
});
