import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group messages', () => {
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

		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.addContactByName('Bob');
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('mygroup');
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
	});

	it('renders messages from other group members with their avatar', async () => {
		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.composer.sendMessage('Hello from Bob!');
		await agent2.groupChatPage.messages.waitForMessage('Hello from Bob!');

		const message = await agent1.groupChatPage.messages.waitForMessage(
			'Hello from Bob!',
		);
		await agent1.waitUntil(
			async () => (await message.authorInitials()) === 'Bo',
			{ timeoutMsg: 'Avatar initials "Bo" did not appear on Bob\'s message' },
		);
	});
});
