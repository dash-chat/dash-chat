import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('pending contact request', () => {
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

	it('hides messages received while the request is pending behind a disclosure', async () => {
		await agent2.directChatPage.composer.sendMessage('hello one');
		await agent2.directChatPage.composer.sendMessage('hello two');

		await agent1.waitUntil(
			async () => (await agent1.homePage.unreadBadge.getText()) === '2',
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg: 'Unread badge never counted the 2 received messages',
			},
		);

		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.ready();
		await agent1.directChatPage.requestMessagesToggle.waitForExist();
		await expect(agent1.directChatPage.requestMessagesToggle).toHaveText(
			'2 messages received',
		);
		expect(
			await agent1.directChatPage.messages.messageAreaContains('hello one'),
		).toBe(false);
	});

	it('reveals and re-hides the messages with the toggle', async () => {
		await agent1.directChatPage.toggleRequestMessages();
		await agent1.directChatPage.messages.waitForMessage('hello one');
		await agent1.directChatPage.messages.waitForMessage('hello two');

		await agent1.directChatPage.toggleRequestMessages();
		await agent1.directChatPage.messages.waitForMessageGone('hello one');
		await agent1.directChatPage.messages.waitForMessageGone('hello two');
	});

	it('keeps the messages unread and collapses again on re-entry', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();

		// Revealing the messages must not mark them read while the request is
		// still pending.
		await agent1.homePage.unreadBadge.waitForDisplayed();
		await expect(agent1.homePage.unreadBadge).toHaveText('2');

		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.ready();
		await agent1.directChatPage.requestMessages.waitForExist();
		expect(
			await agent1.directChatPage.messages.messageAreaContains('hello one'),
		).toBe(false);
	});

	it('shows the messages normally after accepting the request', async () => {
		await agent1.directChatPage.acceptContactRequest();
		await agent1.directChatPage.messages.waitForMessage('hello one');
		await agent1.directChatPage.messages.waitForMessage('hello two');
		await agent1.waitUntil(
			async () => !(await agent1.directChatPage.requestMessages.isExisting()),
		);

		// With the request accepted, viewing the messages marks them read.
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.unreadBadge.waitForDisplayed({ reverse: true });
	});
});
