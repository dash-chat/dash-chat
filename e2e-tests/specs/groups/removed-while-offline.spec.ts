import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

const GROUP = 'Churning group';

/**
 * While a member is unreachable, two admins change the membership in turns,
 * so each change depends on one in the other admin's log, and the last one
 * removes that member. Catching up, it is handed each admin's log whole, so
 * whichever comes first holds a change whose dependency is still missing.
 */
describe('A member removed while offline, after admins took turns', () => {
	let ann: Agent;
	let rob: Agent;
	let cat: Agent;
	let dan: Agent;

	async function removeMember(admin: Agent, name: string): Promise<void> {
		await admin.groupInfoPage.memberItem(name).click();
		await admin.groupInfoPage.removeMemberButton.waitForExist();
		await admin.groupInfoPage.removeMemberButton.click();
		await admin.groupInfoPage.removeMemberConfirmButton.waitForExist();
		await admin.groupInfoPage.removeMemberConfirmButton.click();
		await admin.groupInfoPage.memberItem(name).waitForExist({ reverse: true });
	}

	before(async function () {
		[rob, ann, cat, dan] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Ann: ann, Rob: rob, Cat: cat, Dan: dan });
		for (const contact of [rob, cat, dan]) {
			await exchangeContacts([ann, contact]);
			await backToHome([ann, contact]);
		}

		await createGroup(ann, GROUP, ['Rob', 'Cat']);
		await ann.groupChatPage.composer.sendMessage('Before the removal');
		for (const member of [rob, cat]) {
			await member.homePage
				.chatListItem(GROUP)
				.waitForExist({ timeout: SYNC_TIMEOUT });
			await member.homePage.chatListItem(GROUP).click();
			await member.groupChatPage.ready();
			await member.groupChatPage.messages.waitForMessage('Before the removal');
		}

		await ann.groupChatPage.infoLink.click();
		await ann.groupInfoPage.ready();
		await ann.groupInfoPage.memberItem('Cat').click();
		await ann.groupInfoPage.makeAdminButton.waitForExist();
		await ann.groupInfoPage.makeAdminButton.click();
		await ann.groupInfoPage.makeAdminConfirmButton.waitForExist();
		await ann.groupInfoPage.makeAdminConfirmButton.click();
		await ann.groupInfoPage.memberAdminChip('Cat').waitForExist();

		await cat.groupChatPage.infoLink.click();
		await cat.groupInfoPage.ready();
		await cat.groupInfoPage
			.memberAdminChip('Cat')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		// Rob has to hold the promotion before going away, or Cat's later change
		// is one he could not apply for a reason of its own.
		await rob.groupChatPage.messages
			.systemMessage('group_member_promoted')
			.waitForExist({ timeout: SYNC_TIMEOUT });
	});

	after(async () => {
		if (rob !== undefined && rob.platform !== 'desktop') await rob.enableWifi();
	});

	it('finds itself removed when it comes back', async () => {
		await rob.groupChatPage.back.click();
		await rob.homePage.ready();
		// A stopped phone app is still woken by every push, one change at a
		// time and in order; without a network it gets them all at once later.
		if (rob.platform !== 'desktop') await rob.disableWifi();
		await rob.stopApp();

		await ann.groupInfoPage.addMembersLink.click();
		await ann.addMembersPage.ready();
		await ann.addMembersPage.addContactByName('Dan');
		await ann.addMembersPage.addButton.click();
		await ann.groupInfoPage.ready();
		await ann.groupInfoPage.memberItem('Dan').waitForExist();

		await cat.groupInfoPage
			.memberItem('Dan')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await removeMember(cat, 'Dan');

		await ann.groupInfoPage
			.memberItem('Dan')
			.waitForExist({ reverse: true, timeout: SYNC_TIMEOUT });
		await removeMember(ann, 'Rob');

		await ann.groupInfoPage.back.click();
		await ann.groupChatPage.ready();
		await ann.groupChatPage.composer.sendMessage('After the removal');
		await ann.groupChatPage.messages.waitForMessage('After the removal');
		await ann.groupChatPage.back.click();
		await ann.homePage.ready();
		await ann.homePage.openChat('Rob');
		await ann.directChatPage.composer.sendMessage('Ping after the removal');
		await ann.directChatPage.back.click();
		await ann.homePage.ready();

		await rob.startApp();
		await rob.homePage.ready();
		// The direct message was sent after every group op, so once it is here
		// they have had their chance to arrive too.
		await rob.homePage.openChat('Ann');
		await rob.directChatPage.messages.waitForMessage('Ping after the removal');
		await rob.directChatPage.back.click();
		await rob.homePage.ready();

		await rob.homePage.chatListItem(GROUP).click();
		await rob.groupChatPage.ready();
		await rob.groupChatPage.messages.waitForMessage('Before the removal');
		await expect(rob.groupChatPage.notMemberNotice).toBeExisting();
		await expect(rob.groupChatPage.composer.messageInput).not.toBeExisting();
		expect(
			await rob.groupChatPage.messages.messageAreaContains('After the removal'),
		).toBe(false);
	});

	describe('with the app in the background', () => {
		let notifications: NotificationHelper;

		before(async function () {
			if (rob.platform === 'desktop' || !pushTestingEnabled()) this.skip();
			notifications = notificationHelperFor(rob);
			await rob.enableWifi();
		});

		it('posts no notification for a message sent after the removal', async () => {
			const marker = `AFTER_REMOVAL_${Date.now()}`;
			await rob.startApp();
			await notifications.clear();
			await rob.backgroundApp();

			await ann.homePage.chatListItem(GROUP).click();
			await ann.groupChatPage.ready();
			await ann.groupChatPage.composer.sendMessage(marker);
			await ann.groupChatPage.messages.waitForMessage(marker);
			await ann.groupChatPage.back.click();
			await ann.homePage.ready();

			const ping = `PING_${Date.now()}`;
			await ann.homePage.openChat('Rob');
			await ann.directChatPage.composer.sendMessage(ping);
			await ann.directChatPage.back.click();
			await ann.homePage.ready();

			await expect(notifications).toHaveDelivered({ title: 'Ann', body: ping });
			await expect(notifications).not.toHaveDelivered({ body: marker });
			await expect(notifications).not.toHaveDelivered({
				title: await rob.tr('youHaveANewMessage'),
			});
		});
	});
});
