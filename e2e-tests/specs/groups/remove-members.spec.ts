import { notificationHelperFor } from '../../helpers/components/notifications';
import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Removing group members', () => {
	let agent1: Agent;
	let agent2: Agent;
	let agent3: Agent;

	before(async function () {
		// Bob first: a run with one phone gives it to the first agent, and it is
		// the removed member's notifications that are checked.
		[agent2, agent1, agent3] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);

		await createProfiles({ Alice: agent1, Bob: agent2, Carol: agent3 });
		await exchangeContacts([agent1, agent2]);
		await backToHome([agent1, agent2]);
		await exchangeContacts([agent1, agent3]);
		await backToHome([agent1, agent3]);
	});

	it('admin can remove a non-admin member', async () => {
		await createGroup(agent1, 'Test Group', ['Bob']);

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
		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();

		// Bob should no longer appear in the members list
		await agent1.groupInfoPage
			.memberItem('Bob')
			.waitForExist({ reverse: true });

		// From bob's point of view, check that they have been removed and see the system message about it
		await agent2.homePage.ready();
		await agent2.homePage.chatListItem('Test Group').click();
		await agent2.groupChatPage.ready();

		const systemMessage = agent2.groupChatPage.messages.systemMessage(
			'group_member_removed',
		);
		await systemMessage.waitForExist({ timeout: SYNC_TIMEOUT });
		await expect(systemMessage).toBeExisting();
		const expectedText = await agent2.tr('someoneRemovedYouFromTheGroup', {
			name: 'Alice',
		});
		await expect(systemMessage).toHaveText(expectedText);

		// Composer is replaced by a notice (no longer a member)
		await expect(agent2.groupChatPage.notMemberNotice).toBeExisting();
		await expect(agent2.groupChatPage.composer.messageInput).not.toBeExisting();

		await agent2.groupChatPage.back.click();
		await agent2.homePage.ready();
	});

	it('removed member sees neither a later membership change nor a later message', async () => {
		await agent1.homePage.chatListItem('Test Group').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.addMembersLink.click();
		await agent1.addMembersPage.ready();
		await agent1.addMembersPage.addContactByName('Carol');
		await agent1.addMembersPage.addButton.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.memberItem('Carol').waitForExist();
		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.composer.sendMessage('After the removal');
		await agent1.groupChatPage.messages.waitForMessage('After the removal');
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		// Nothing may arrive in the group for Bob, so there is no group op to
		// wait on: a direct message sent after the group ops is the barrier that
		// says they had their chance to reach him.
		await agent1.homePage.openChat('Bob');
		await agent1.directChatPage.composer.sendMessage('Ping after the removal');
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.openChat('Alice');
		await agent2.directChatPage.messages.waitForMessage(
			'Ping after the removal',
		);
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();

		const chatRow = await agent2.homePage.chatListItem('Test Group').getText();
		expect(chatRow).not.toContain('After the removal');
		expect(chatRow).not.toContain('Carol');

		await agent2.homePage.chatListItem('Test Group').click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.messages
			.systemMessage('group_member_removed')
			.waitForExist();
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				'After the removal',
			),
		).toBe(false);
		await expect(
			agent2.groupChatPage.messages.systemMessage('group_member_added'),
		).not.toBeExisting();
		await agent2.groupChatPage.infoLink.click();
		await agent2.groupInfoPage.ready();
		await agent2.groupInfoPage.name.waitForExist();
		await expect(agent2.groupInfoPage.members).not.toBeExisting();
		await agent2.groupInfoPage.back.click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.back.click();
		await agent2.homePage.ready();
	});

	it('removed member gets no notification for a later message', async function () {
		if (agent2.platform === 'desktop' || !pushTestingEnabled()) this.skip();
		const notifications = notificationHelperFor(agent2);
		const marker = `AFTER_REMOVAL_${Date.now()}`;
		// Read from the webview, which the session is off while it reads
		// Notification Center for a backgrounded app.
		const newMessage = await agent2.tr('youHaveANewMessage');
		await notifications.clear();
		await agent2.backgroundApp();

		await agent1.homePage.chatListItem('Test Group').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.composer.sendMessage(marker);
		await agent1.groupChatPage.messages.waitForMessage(marker);
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		const ping = `PING_${Date.now()}`;
		await agent1.homePage.openChat('Bob');
		await agent1.directChatPage.composer.sendMessage(ping);
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();

		try {
			await expect(notifications).toHaveDelivered({
				title: 'Alice',
				body: ping,
			});
			await expect(notifications).not.toHaveDelivered({ body: marker });
			await expect(notifications).not.toHaveDelivered({ title: newMessage });
		} finally {
			await agent2.startApp();
			await agent2.homePage.ready();
		}
	});

	it('removed member takes part again once added back, without what it missed', async () => {
		await agent1.homePage.chatListItem('Test Group').click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.addMembersLink.click();
		await agent1.addMembersPage.ready();
		await agent1.addMembersPage.addContactByName('Bob');
		await agent1.addMembersPage.addButton.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.memberItem('Bob').waitForExist();
		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.composer.sendMessage('After coming back');
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();

		await agent2.homePage.chatListItem('Test Group').click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.messages.waitForMessage('After coming back');
		await expect(agent2.groupChatPage.composer.messageInput).toBeExisting();
		await expect(agent2.groupChatPage.notMemberNotice).not.toBeExisting();
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				'After the removal',
			),
		).toBe(false);
		expect(
			await agent2.groupChatPage.messages.messageAreaContains('Carol'),
		).toBe(false);
		await agent2.groupChatPage.back.click();
		await agent2.homePage.ready();
	});

	it('non-admin cannot remove another member', async () => {
		await createGroup(agent1, 'Test Group 2', ['Bob']);

		// Bob navigates to the group
		await agent2.homePage.ready();
		await agent2.homePage.chatListItem('Test Group 2').waitForExist();
		await agent2.homePage.chatListItem('Test Group 2').click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.infoLink.click();
		await agent2.groupInfoPage.ready();

		// Non-admins see member items without an onclick handler — clicking does nothing
		await agent2.groupInfoPage.memberItem('Alice').waitForExist();
		await agent2.groupInfoPage.memberItem('Alice').click();

		// The remove member button should not appear
		await expect(agent2.groupInfoPage.removeMemberButton).not.toBeExisting();

		await agent2.groupInfoPage.back.click();
		await agent2.groupChatPage.ready();
		await agent2.groupChatPage.back.click();
	});
});
