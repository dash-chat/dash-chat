import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Group unread messages', () => {
	const REQUIRED_OVERFLOW = 400;
	const MAX_FILLER = 200;

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		this.timeout(120_000);
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('fills the group chat until it overflows enough to scroll', async () => {
		let i = 0;
		let overflow = await agent1.groupChatPage.scroll.overflow();
		while (overflow < REQUIRED_OVERFLOW && i < MAX_FILLER) {
			await agent1.groupChatPage.sendMessage(`filler ${i}`);
			await agent1.groupChatPage.waitForMessage(`filler ${i}`, 10_000);
			overflow = await agent1.groupChatPage.scroll.overflow();
			i++;
		}
		expect(overflow).toBeGreaterThanOrEqual(REQUIRED_OVERFLOW);
		await agent1.waitUntil(async () => agent1.groupChatPage.scroll.isAtBottom());
		await agent2.groupChatPage.waitForMessage(`filler ${i - 1}`);
		await agent2.waitUntil(async () => agent2.groupChatPage.scroll.isAtBottom());
	});

	it('shows unread badge when a peer message arrives while scrolled up', async () => {
		await agent2.groupChatPage.scroll.scrollUp();
		expect(await agent2.groupChatPage.scroll.isAtBottom()).toBe(false);

		await agent1.groupChatPage.sendMessage('peer while scrolled up');
		await agent2.groupChatPage.waitForMessage('peer while scrolled up');

		expect(await agent2.groupChatPage.scroll.isAtBottom()).toBe(false);
		expect(await agent2.groupChatPage.scrollBottomButtonVisible()).toBe(true);
		expect(await agent2.groupChatPage.unreadBadgeText()).toBeTruthy();
	});

	it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
		await agent2.groupChatPage.scroll.scrollUp();
		await agent1.groupChatPage.sendMessage('unread badge precondition');
		await agent2.groupChatPage.waitForMessage('unread badge precondition');
		await agent2.waitUntil(
			async () => (await agent2.groupChatPage.unreadBadgeText()) !== null,
		);
		expect(await agent2.groupChatPage.scrollBottomButtonVisible()).toBe(true);

		await agent2.groupChatPage.clickScrollBottomButton();

		await agent2.waitUntil(async () => agent2.groupChatPage.scroll.isAtBottom());
		await agent2.waitUntil(
			async () => (await agent2.groupChatPage.unreadBadgeText()) === null,
		);
	});
});
