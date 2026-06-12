import { tid } from '../../helpers/selectors';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Leaving group', () => {
	let agent1: Agent;

	before(async () => {
		agent1 = await setupAgent('agent1');
		await agent1.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
	});

	it('creator can leave a group they created alone', async () => {
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('Solo Group');
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();

		await agent1.groupInfoPage.leaveButton.click();
		await agent1.groupInfoPage.leaveConfirmButton.waitForExist();
		await agent1.groupInfoPage.leaveConfirmButton.click();

		await agent1.homePage.ready();

		// Group remains in chat list
		await expect(agent1.homePage.chatListItem('Solo Group')).toBeExisting();

		// Navigate back into the group
		await agent1.homePage.chatListItem('Solo Group').click();
		await agent1.groupChatPage.ready();

		// Message input is disabled (no longer a member)
		await expect(agent1.groupChatPage.messageInput).not.toBeEnabled();

		// System message records the departure
		const systemMessage = agent1.$(
			'[data-testid="group-chat-system-message-group_member_removed"]',
		);
		await expect(systemMessage).toBeExisting();
		const expectedText = await agent1.tr('youLeftTheGroup');
		await expect(systemMessage).toHaveText(expectedText);

		// Leave button is gone — already left, and Alice no longer in members list
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await expect(agent1.groupInfoPage.leaveButton).not.toBeDisplayed();
		const membersList = agent1.$(tid('group-info-members'));
		await expect(membersList.$('=Alice')).not.toBeExisting();
	});
});
