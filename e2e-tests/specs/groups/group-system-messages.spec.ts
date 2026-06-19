import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Group chat inline system messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
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
		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.systemMessage('group_created').waitForExist();
		const added = await agent2.groupChatPage
			.systemMessage('group_created')
			.getText();
		expect(added).toContain('Alice Test added you to the group.');
	});

	it('renders regular chat bubbles separately from system messages', async () => {
		await agent1.groupChatPage.sendMessage('Hi everyone');
		await agent1.groupChatPage.messages.waitForMessage('Hi everyone');

		const bubble = await agent1.groupChatPage.messages.messageBubbleWithText(
			'Hi everyone',
		);
		expect(bubble).not.toBeNull();
	});
});
