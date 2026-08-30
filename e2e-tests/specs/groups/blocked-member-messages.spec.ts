import { blockAgent } from '../../helpers/flows/block-agent';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

const GROUP = 'mygroup';

describe('Blocked group member', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		// The group has to exist before the block: a blocked contact is hidden
		// from the member pickers, so Bob could not be added afterwards.
		await createGroup(agent1, GROUP, 'Bob');
		await agent2.homePage
			.chatListItem(GROUP)
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await agent2.homePage.chatListItem(GROUP).click();
		await agent2.groupChatPage.ready();
	});

	it('hides the group messages a member sends while blocked', async () => {
		await agent2.groupChatPage.composer.sendMessage('Before block');
		await agent1.groupChatPage.messages.waitForMessage('Before block');

		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.openChat('Bob Test');
		await blockAgent(agent1);
		await agent1.directChatPage.blockedBanner.waitForDisplayed();
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem(GROUP).click();
		await agent1.groupChatPage.ready();

		await agent2.groupChatPage.composer.sendMessage('Sent while blocked');
		await agent2.groupChatPage.messages.waitForMessage('Sent while blocked');
		expect(
			await agent1.groupChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);
	});

	it('keeps it hidden after unblocking and across a restart', async () => {
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.unblockButton.click();
		await agent1.directChatPage.unblockConfirm.waitForClickable();
		await agent1.directChatPage.unblockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed({
			reverse: true,
		});
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem(GROUP).click();
		await agent1.groupChatPage.ready();

		// Bob's log is sequential, so once this later message arrives the one
		// sent while blocked has also been synced — and rejected — by Alice.
		await agent2.groupChatPage.composer.sendMessage('After unblock');
		await agent1.groupChatPage.messages.waitForMessage('After unblock');
		expect(
			await agent1.groupChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);

		// A cold start rebuilds the message list from the raw op store, which
		// holds the blocked-time op the backend rejected — the message must
		// stay hidden on this path too, even now that Bob is unblocked.
		await agent1.restart();
		await agent1.homePage.ready();
		await agent1.homePage.chatListItem(GROUP).click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.messages.waitForMessage('After unblock');
		expect(
			await agent1.groupChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);
	});
});
