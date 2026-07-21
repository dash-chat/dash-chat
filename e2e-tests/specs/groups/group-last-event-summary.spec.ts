import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group chat list last-event summary', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
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

	it('shows "You created the group." for the creator of a fresh group', async () => {
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('mygroup');
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		const row = await agent1.homePage.chatRowText('mygroup');
		expect(row).toContain('You created the group.');
	});

	it('shows "Member added." after a member is added to the group', async () => {
		await agent1.homePage.chatListItem('mygroup').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.addMembersLink.click();

		await agent1.addMembersPage.ready();
		await agent1.addMembersPage.addContactByName('Bob');
		await agent1.addMembersPage.addButton.click();

		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		const aliceRow = await agent1.homePage.chatRowText('mygroup');
		expect(aliceRow).toContain('You added Bob Test.');

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		const bobRow = await agent2.homePage.chatRowText('mygroup');
		expect(bobRow).toContain('Alice Test added you to the group.');
	});

	it('shows the latest message text once a message is sent', async () => {
		await agent1.homePage.chatListItem('mygroup').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.sendMessage('Hello group!');
		await agent1.groupChatPage.messages.waitForMessage('Hello group!');
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		const aliceRow = await agent1.homePage.chatRowText('mygroup');
		expect(aliceRow).toContain('Alice Test');
		expect(aliceRow).toContain('Hello group!');
		expect(aliceRow).not.toContain('added.');

		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.messages.waitForMessage('Hello group!');
		await agent2.groupChatPage.back.click();
		await agent2.homePage.ready();

		const bobRow = await agent2.homePage.chatRowText('mygroup');
		expect(bobRow).toContain('Alice Test');
		expect(bobRow).toContain('Hello group!');
	});
});
