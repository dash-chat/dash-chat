import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('New group', () => {
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
	});

	it('creates a new group with no members except the creator', async () => {
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem('mygroup').waitForExist();
	});

	it('creates a new group with another member', async () => {
		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.addContactByName('Bob');

		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem('mygroup').waitForExist();

		await agent2.homePage.chatListItem('mygroup').waitForExist();
	});
});
