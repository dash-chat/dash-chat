import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import {
	MAILBOX_HEALED_AFTER_BACKOFF_MS,
	SYNC_TIMEOUT,
} from '../../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';

const GROUP_NAME = 'P2P Group';

describe('Group presence over p2p', function () {
	this.timeout(300_000);

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		suspendMailbox();
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
		await backToHome([agent1, agent2]);
		await createGroup(agent1, GROUP_NAME, ['Bob']);
	});

	it('counts the connected people and badges the group row, only while reachable just over p2p', async () => {
		const { groupChatPage, homePage } = agent1;
		await groupChatPage.connectedPeersCount.waitForDisplayed({
			timeout: SYNC_TIMEOUT,
		});
		expect(await groupChatPage.connectedPeersCount.getText()).toBe(
			await agent1.tr('connectedPeersCount', { count: 1 }),
		);

		await groupChatPage.back.click();
		await homePage.ready();
		const badge = homePage.chatRowP2pBadge(GROUP_NAME);
		await badge.waitForDisplayed({ timeout: SYNC_TIMEOUT });

		resumeMailbox();
		await badge.waitForExist({
			reverse: true,
			timeout: MAILBOX_HEALED_AFTER_BACKOFF_MS,
		});

		await homePage.chatListItem(GROUP_NAME).click();
		await groupChatPage.ready();
		await groupChatPage.connectedPeersCount.waitForExist({ reverse: true });
	});
});
