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
		await expect(agent1.homePage.chatListItem('Solo Group')).not.toBeExisting();
	});
});
