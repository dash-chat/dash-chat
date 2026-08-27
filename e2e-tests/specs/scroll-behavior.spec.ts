/**
 * Chat scroll behavior E2E tests.
 *
 * Verifies the reverse-scroll container behavior in a direct chat:
 * staying pinned at bottom, the scroll-to-bottom button + unread badge,
 * and the transparent navbar's opacity transitions on scroll.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { UI_TIMEOUT } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Chat scroll behavior', () => {
	// Need enough overflow that scrollUp can move past the bottom
	// threshold (200px) — leave headroom so timing/layout jitter doesn't
	// drop us below.
	const REQUIRED_OVERFLOW = 400;
	const MAX_FILLER = 200;

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('fills the chat until it overflows enough to scroll', async () => {
		let i = 0;
		let overflow = await agent1.directChatPage.scroll.overflow();
		while (overflow < REQUIRED_OVERFLOW && i < MAX_FILLER) {
			await agent1.directChatPage.composer.sendMessage(`filler ${i}`);
			await agent1.directChatPage.messages.waitForMessage(
				`filler ${i}`,
				UI_TIMEOUT,
			);
			overflow = await agent1.directChatPage.scroll.overflow();
			i++;
		}
		expect(overflow).toBeGreaterThanOrEqual(REQUIRED_OVERFLOW);
		await agent1.waitUntil(async () =>
			agent1.directChatPage.scroll.isAtBottom(),
		);
	});

	it('returns to bottom when the user sends while scrolled up', async () => {
		await agent1.directChatPage.scroll.scrollUp();
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(false);

		await agent1.directChatPage.composer.sendMessage(
			'self-send after scroll up',
		);
		await agent1.directChatPage.messages.waitForMessage(
			'self-send after scroll up',
		);

		await agent1.waitUntil(async () =>
			agent1.directChatPage.scroll.isAtBottom(),
		);
	});

	it('stays pinned to bottom when a peer message arrives at bottom', async () => {
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(true);

		await agent2.directChatPage.composer.sendMessage('peer at bottom');
		await agent1.directChatPage.messages.waitForMessage('peer at bottom');

		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(true);
	});

	it('does not auto-scroll when a peer message arrives while scrolled up', async () => {
		await agent1.directChatPage.scroll.scrollUp();
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(false);

		await agent2.directChatPage.composer.sendMessage('peer while scrolled up');
		await agent1.directChatPage.messages.waitForMessage(
			'peer while scrolled up',
		);

		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(false);
		expect(await agent1.directChatPage.messages.scrollBottom.isExisting()).toBe(
			true,
		);
		expect(await agent1.directChatPage.messages.unreadBadgeText()).toBeTruthy();
	});

	it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
		await agent1.directChatPage.scroll.scrollUp();
		await agent2.directChatPage.composer.sendMessage(
			'unread badge precondition',
		);
		await agent1.directChatPage.messages.waitForMessage(
			'unread badge precondition',
		);
		await agent1.waitUntil(
			async () =>
				(await agent1.directChatPage.messages.unreadBadgeText()) !== null,
		);
		expect(await agent1.directChatPage.messages.scrollBottom.isExisting()).toBe(
			true,
		);

		await agent1.directChatPage.messages.scrollBottom.click();

		await agent1.waitUntil(async () =>
			agent1.directChatPage.scroll.isAtBottom(),
		);
		await agent1.waitUntil(
			async () =>
				(await agent1.directChatPage.messages.unreadBadgeText()) === null,
		);
	});

	it('hides the scroll-to-bottom button once the user scrolls back down', async () => {
		await agent1.directChatPage.scroll.scrollUp();
		expect(await agent1.directChatPage.messages.scrollBottom.isExisting()).toBe(
			true,
		);

		await agent1.directChatPage.scroll.scrollToBottom();
		await agent1.waitUntil(
			async () =>
				!(await agent1.directChatPage.messages.scrollBottom.isExisting()),
		);
	});

	// Guards against silent regressions in the Konsta selector inside
	// ReverseScrollPage. The transparent navbar's bg should be opaque
	// (opacity '1') at the bottom — where the latest message sits right
	// under the navbar — and fade out ('0') only once the user scrolls
	// all the way to the welcome / avatar surface at the top of the chat.
	it('toggles transparent navbar opacity on scroll', async () => {
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(true);
		await agent1.waitUntil(
			async () =>
				(await agent1.directChatPage.scroll.navbarBgOpacity()) === '1',
		);

		await agent1.directChatPage.scroll.scrollToTop();
		await agent1.waitUntil(
			async () =>
				(await agent1.directChatPage.scroll.navbarBgOpacity()) === '0',
		);

		await agent1.directChatPage.scroll.scrollToBottom();
		await agent1.waitUntil(
			async () =>
				(await agent1.directChatPage.scroll.navbarBgOpacity()) === '1',
		);
	});
});
