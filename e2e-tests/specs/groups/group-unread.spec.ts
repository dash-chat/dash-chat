import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT, UI_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group unread messages', () => {
	const REQUIRED_OVERFLOW = 400;
	const MAX_FILLER = 200;

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);

		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('fills the group chat until it overflows enough to scroll', async () => {
		let i = 0;
		// agent2 does the scrolling below, and viewports differ across
		// platforms — keep filling until BOTH containers overflow enough.
		const minOverflow = async () =>
			Math.min(
				await agent1.groupChatPage.scroll.overflow(),
				await agent2.groupChatPage.scroll.overflow(),
			);
		let overflow = await minOverflow();
		while (overflow < REQUIRED_OVERFLOW && i < MAX_FILLER) {
			await agent1.groupChatPage.composer.sendMessage(`filler ${i}`);
			await agent1.groupChatPage.messages.waitForMessage(
				`filler ${i}`,
				UI_TIMEOUT,
			);
			overflow = await minOverflow();
			i++;
		}
		expect(overflow).toBeGreaterThanOrEqual(REQUIRED_OVERFLOW);
		await agent1.waitUntil(async () =>
			agent1.groupChatPage.scroll.isAtBottom(),
		);
		await agent2.groupChatPage.messages.waitForMessage(`filler ${i - 1}`);
		await agent2.waitUntil(async () =>
			agent2.groupChatPage.scroll.isAtBottom(),
		);
	});

	it('shows unread badge when a peer message arrives while scrolled up', async () => {
		await agent2.groupChatPage.scroll.scrollUp();
		expect(await agent2.groupChatPage.scroll.isAtBottom()).toBe(false);

		await agent1.groupChatPage.composer.sendMessage('peer while scrolled up');
		await agent2.groupChatPage.messages.waitForMessage(
			'peer while scrolled up',
		);

		expect(await agent2.groupChatPage.scroll.isAtBottom()).toBe(false);
		expect(await agent2.groupChatPage.messages.scrollBottom.isExisting()).toBe(
			true,
		);
		expect(await agent2.groupChatPage.messages.unreadBadgeText()).toBeTruthy();

		await agent2.groupChatPage.messages.unreadDivider.waitForExist();
		const message = await agent2.groupChatPage.messages.waitForMessage(
			'peer while scrolled up',
		);
		expect(await message.isPrecededByUnreadDivider()).toBe(true);
	});

	it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
		await agent2.groupChatPage.scroll.scrollUp();
		await agent1.groupChatPage.composer.sendMessage(
			'unread badge precondition',
		);
		await agent2.groupChatPage.messages.waitForMessage(
			'unread badge precondition',
		);
		await agent2.waitUntil(
			async () =>
				(await agent2.groupChatPage.messages.unreadBadgeText()) !== null,
		);
		expect(await agent2.groupChatPage.messages.scrollBottom.isExisting()).toBe(
			true,
		);

		await agent2.groupChatPage.messages.scrollBottom.click();

		await agent2.waitUntil(async () =>
			agent2.groupChatPage.scroll.isAtBottom(),
		);
		await agent2.waitUntil(
			async () =>
				(await agent2.groupChatPage.messages.unreadBadgeText()) === null,
		);
	});
});
