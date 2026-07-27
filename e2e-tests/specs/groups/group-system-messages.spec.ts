import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group chat inline system messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
	});

	it('renders "You created the group." for the creator', async () => {
		await agent1.groupChatPage.systemMessage('group_created').waitForExist();
		const created = await agent1.groupChatPage
			.systemMessage('group_created')
			.getText();
		expect(created).toContain('You created the group.');
	});

	it('renders "{creator} added you to the group." for the invited member', async () => {
		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.systemMessage('group_created').waitForExist();
		const added = await agent2.groupChatPage
			.systemMessage('group_created')
			.getText();
		expect(added).toContain('Alice Test added you to the group.');
	});

	it('renders regular chat bubbles separately from system messages', async () => {
		await agent1.groupChatPage.composer.sendMessage('Hi everyone');
		const message =
			await agent1.groupChatPage.messages.waitForMessage('Hi everyone');

		expect(await message.wrapper.isExisting()).toBe(true);
	});
});
