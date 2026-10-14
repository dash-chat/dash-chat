import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT, UI_TIMEOUT } from '../../helpers/timeouts';
import { isRemoteMailbox, suspendMailbox } from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Adding a member the other members are not contacts with', function () {
	this.timeout(300_000);

	let admin: Agent;
	let member: Agent;
	let newcomer: Agent;

	before(async function () {
		[admin, member, newcomer] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Ann: admin, Ben: member, Cat: newcomer });
		await exchangeContacts([admin, member]);
		await backToHome([admin, member]);
		await exchangeContacts([admin, newcomer]);
		await backToHome([admin, newcomer]);
		await createGroup(admin, 'mygroup', ['Ben']);
	});

	it('names the newcomer in the member list a member already has open', async () => {
		await member.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await member.homePage.chatListItem('mygroup').click();
		await member.groupChatPage.ready();
		await member.groupChatPage.infoLink.click();
		await member.groupInfoPage.ready();

		await admin.groupChatPage.infoLink.click();
		await admin.groupInfoPage.ready();
		await admin.groupInfoPage.addMembersLink.click();
		await admin.addMembersPage.ready();
		await admin.addMembersPage.addContactByName('Cat');
		await admin.addMembersPage.addButton.click();
		await admin.groupInfoPage.ready();

		await member.groupInfoPage
			.memberItem('Cat')
			.waitForExist({ timeout: SYNC_TIMEOUT });
	});

	it('reads p2p in the group chip when only the newcomer is connected', async function () {
		if (isRemoteMailbox()) this.skip();
		await newcomer.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		suspendMailbox();
		await admin.stopApp();
		// Relaunched, the member's only possible session on the group is with
		// the newcomer, now that the admin is offline.
		await member.stopApp();
		await member.startApp();
		await member.homePage.ready();
		await member.homePage.chatListItem('mygroup').click();
		await member.groupChatPage.ready();

		await member.groupChatPage.connectionStatusIndicator.waitForStatus(
			'p2p',
			UI_TIMEOUT,
			'the group chip did not read p2p with only a non-contact member connected',
		);
	});
});
