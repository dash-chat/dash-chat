import { avatarAppearance } from '../../helpers/components/avatar';
import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { tid } from '../../helpers/selectors';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Group details spec', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
	});

	it('Renders a group glyph, not initials, when the group has no photo', async () => {
		await agent1.groupChatPage.headerName.waitForExist();
		const avatar = await avatarAppearance(agent1, tid('group-chat-header'));
		expect(avatar.glyph).toBe(true);
	});

	it('Shows a new name to all members if the admin changes it', async () => {
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.editLink.click();
		await agent1.groupInfoEditPage.ready();
		await agent1.groupInfoEditPage.setName('renamed group');
		await agent1.groupInfoEditPage.save();
		await agent1.groupInfoPage.ready();

		await agent2.homePage.chatListItem('renamed group').waitForExist();
		await agent2.homePage.chatListItem('renamed group').click();
		await agent2.groupChatPage.ready();
		await agent2.waitUntil(async () =>
			(await agent2.groupChatPage.headerName.getText()).includes(
				'renamed group',
			),
		);
	});

	it('Hides the edit details link from non-admins', async () => {
		await agent2.groupChatPage.infoLink.click();
		await agent2.groupInfoPage.ready();

		await expect(agent2.groupInfoPage.editLink).not.toBeDisplayed();
	});
});
