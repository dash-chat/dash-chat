import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Making a group member an admin', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);

		await exchangeContactsAndCreateGroup({ Alice: agent1, Bob: agent2 });
	});

	it('admin makes a member an admin', async () => {
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();

		await agent1.groupInfoPage.memberItem('Bob').waitForExist();
		await expect(
			agent1.groupInfoPage.memberAdminChip('Bob'),
		).not.toBeExisting();

		await agent1.groupInfoPage.memberItem('Bob').click();
		await agent1.groupInfoPage.makeAdminButton.waitForExist();
		await agent1.groupInfoPage.makeAdminButton.click();
		await agent1.groupInfoPage.makeAdminConfirmButton.waitForExist();
		await agent1.groupInfoPage.makeAdminConfirmButton.click();

		await agent1.groupInfoPage.memberAdminChip('Bob').waitForExist();

		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		const youMadeBobAdmin = agent1.groupChatPage.messages.systemMessage(
			'group_member_promoted',
		);
		await youMadeBobAdmin.waitForExist();
		await expect(youMadeBobAdmin).toHaveText(
			await agent1.tr('youMadeMemberAdmin', { name: 'Bob' }),
		);

		await agent2.homePage.ready();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
		const aliceMadeYouAdmin = agent2.groupChatPage.messages.systemMessage(
			'group_member_promoted',
		);
		await aliceMadeYouAdmin.waitForExist({ timeout: SYNC_TIMEOUT });
		await expect(aliceMadeYouAdmin).toHaveText(
			await agent2.tr('someoneMadeYouAnAdmin', { name: 'Alice' }),
		);

		await agent2.groupChatPage.infoLink.click();
		await agent2.groupInfoPage.ready();
		await agent2.groupInfoPage.memberAdminChip('Bob').waitForExist();
		await expect(agent2.groupInfoPage.addMembersLink).toBeExisting();
		await expect(agent2.groupInfoPage.editLink).toBeExisting();
	});
});
