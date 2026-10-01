import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Adding a member the other members are not contacts with', () => {
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
});
