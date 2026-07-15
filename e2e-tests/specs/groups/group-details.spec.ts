import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group details spec', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		({ agent1, agent2 } = await setupAgents(this, {
			agent1: 'any',
			agent2: 'any',
		}));
		await exchangeContactsAndCreateGroup(agent1, agent2);
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
