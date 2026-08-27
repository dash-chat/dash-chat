import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('block a pending contact request', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		// One-way add, so Alice lands on a chat showing Bob's contact request.
		await navigateToAddContact(agent1);
		const aliceLink = await agent1.addContactPage.getAddContactLink();
		await agent1.addContactPage.back.click();
		await agent1.newMessagePage.back.click();
		await agent1.homePage.ready();

		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterAddContactLink(aliceLink);
		await agent2.directChatPage.ready();
	});

	it('shows the request with an unread badge', async () => {
		await agent1.homePage.chatList.waitForExist();
		await agent1.homePage.unreadBadge.waitForDisplayed();
		await expect(agent1.homePage.unreadBadge).toHaveText('1');
	});

	it('clears the badge when the request is blocked', async () => {
		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.blockButton.waitForClickable();
		await agent1.directChatPage.blockButton.click();
		await agent1.directChatPage.blockConfirm.waitForClickable();
		await agent1.directChatPage.blockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed();

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.blockedRowIcon.waitForDisplayed();
		await agent1.homePage.unreadBadge.waitForDisplayed({ reverse: true });
	});

	it('restores the badge when the contact is unblocked', async () => {
		await agent1.homePage.chatRow.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.unblockButton.click();
		await agent1.directChatPage.unblockConfirm.waitForClickable();
		await agent1.directChatPage.unblockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed({
			reverse: true,
		});

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.unreadBadge.waitForDisplayed();
		await expect(agent1.homePage.unreadBadge).toHaveText('1');
	});
});
