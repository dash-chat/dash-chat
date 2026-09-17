import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfilesAndExchangeContacts } from '../../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group chat list last-event summary', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfilesAndExchangeContacts({ Alice: agent1, Bob: agent2 });
		await backToHome([agent1, agent2]);
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
		expect(row).toContain(await agent1.tr('youCreatedTheGroup'));
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
		expect(aliceRow).toContain(
			await agent1.tr('youAddedMember', { name: 'Bob' }),
		);

		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		const bobRow = await agent2.homePage.chatRowText('mygroup');
		expect(bobRow).toContain(
			await agent2.tr('someoneAddedYouToTheGroup', { name: 'Alice' }),
		);
	});

	it('shows the latest message text once a message is sent', async () => {
		await agent1.homePage.chatListItem('mygroup').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.composer.sendMessage('Hello group!');
		await agent1.groupChatPage.messages.waitForMessage('Hello group!');
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		const aliceRow = await agent1.homePage.chatRowText('mygroup');
		expect(aliceRow).toContain('Alice');
		expect(aliceRow).toContain('Hello group!');
		expect(aliceRow).not.toContain('added.');

		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.messages.waitForMessage('Hello group!');
		await agent2.groupChatPage.back.click();
		await agent2.homePage.ready();

		const bobRow = await agent2.homePage.chatRowText('mygroup');
		expect(bobRow).toContain('Alice');
		expect(bobRow).toContain('Hello group!');
	});
});
