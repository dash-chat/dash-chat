import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup({ Alice: agent1, Bob: agent2 });
	});

	it('renders messages from other group members with their avatar', async () => {
		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.composer.sendMessage('Hello from Bob!');
		await agent2.groupChatPage.messages.waitForMessage('Hello from Bob!');

		const message =
			await agent1.groupChatPage.messages.waitForMessage('Hello from Bob!');
		await agent1.waitUntil(
			async () => (await message.authorInitials()) === 'Bo',
			{ timeoutMsg: 'Avatar initials "Bo" did not appear on Bob\'s message' },
		);
	});

	it('marks a group message delivered once another member receives it', async () => {
		await agent1.groupChatPage.composer.sendMessage('Hello group!');
		await agent2.groupChatPage.messages.waitForMessage('Hello group!');
		await agent1.groupChatPage.messages.waitForMessageStatus('Hello group!', [
			'delivered',
		]);
	});
});
