import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('New group', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: agent1, Bob: agent2 });
	});

	it('creates a new group with no members except the creator', async () => {
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('Solo Group');
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem('Solo Group').waitForExist();
	});

	it('creates a new group with another member', async () => {
		await exchangeContacts([agent1, agent2]);
		await backToHome([agent1, agent2]);

		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.addContactByName('Bob');

		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('Family');
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem('Family').waitForExist();

		await agent2.homePage.chatListItem('Family').waitForExist();
	});
});
