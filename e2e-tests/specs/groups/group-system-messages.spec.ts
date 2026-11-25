import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group chat inline system messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup({ Alice: agent1, Bob: agent2 });
	});

	it('renders "You created the group." for the creator', async () => {
		await agent1.groupChatPage.messages
			.systemMessage('group_created')
			.waitForExist();
		const created = await agent1.groupChatPage.messages
			.systemMessage('group_created')
			.getText();
		expect(created).toContain(await agent1.tr('youCreatedTheGroup'));
	});

	it('renders "{creator} added you to the group." for the invited member', async () => {
		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.messages
			.systemMessage('group_created')
			.waitForExist();
		const added = await agent2.groupChatPage.messages
			.systemMessage('group_created')
			.getText();
		expect(added).toContain(
			await agent2.tr('someoneAddedYouToTheGroup', { name: 'Alice' }),
		);
	});

	it('renders regular chat bubbles separately from system messages', async () => {
		await agent1.groupChatPage.composer.sendMessage('Hi everyone');
		const message =
			await agent1.groupChatPage.messages.waitForMessage('Hi everyone');

		expect(await message.wrapper.isExisting()).toBe(true);
	});

	it("follows a member's rename in the messages it already shows", async () => {
		expect(
			await agent2.groupChatPage.messages.messageAreaContains(
				await agent2.tr('someoneAddedYouToTheGroup', { name: 'Alice' }),
			),
		).toBe(true);

		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.mainTabbar.settingsTab.click();
		await agent1.settingsPage.ready();
		await agent1.settingsPage.profileLink.click();
		await agent1.profilePage.ready();
		await agent1.profilePage.editName.click();
		await agent1.editNamePage.ready();
		await agent1.editNamePage.setName('Alicia');
		await agent1.editNamePage.save();
		await agent1.profilePage.ready();

		const renamed = await agent2.tr('someoneAddedYouToTheGroup', {
			name: 'Alicia',
		});
		await agent2.waitUntil(
			async () => agent2.groupChatPage.messages.messageAreaContains(renamed),
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg: `the group never showed "${renamed}"`,
			},
		);
	});
});
