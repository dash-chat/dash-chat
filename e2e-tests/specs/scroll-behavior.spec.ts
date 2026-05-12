/**
 * Chat scroll behavior E2E tests.
 *
 * Verifies the reverse-scroll container behavior in a direct chat:
 * staying pinned at bottom, the scroll-to-bottom button + unread badge,
 * and the transparent navbar's opacity transitions on scroll.
 *
 * Uses two Tauri instances (agent1 & agent2) via WebdriverIO multiremote
 * and calls window.__test functions registered by ui/tests/setup-utils.ts.
 */

import {
	type Agent,
	exchangeContacts,
	setupAgent,
} from '../helpers/setup-agents';

describe('Chat scroll behavior', () => {
	// Need enough overflow that scrollChatUp can move past the bottom
	// threshold (200px) — leave headroom so timing/layout jitter doesn't
	// drop us below.
	const REQUIRED_OVERFLOW = 400;
	// Hard cap so a misconfigured viewport can't loop forever.
	const MAX_FILLER = 200;

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		this.timeout(120_000);
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfile('Alice', 'Test');
		await agent2.createProfile('Bob', 'Test');
		// addContact (inside exchangeContacts) lands agent1 on the direct chat
		// with Bob, which is exactly where the scroll tests need to start.
		await exchangeContacts(agent1, agent2);
	});

	it('fills the chat until it overflows enough to scroll', async () => {
		let i = 0;
		let overflow = await agent1.chatOverflow();
		while (overflow < REQUIRED_OVERFLOW && i < MAX_FILLER) {
			await agent1.sendMessage(`filler ${i}`);
			// Wait for the message to render before measuring — DOM updates
			// are async after the click.
			await agent1.waitForMessage(`filler ${i}`, 10_000);
			overflow = await agent1.chatOverflow();
			i++;
		}
		expect(overflow).toBeGreaterThanOrEqual(REQUIRED_OVERFLOW);
		await agent1.waitUntil(async () => agent1.isScrollAtBottom(), {
			timeout: 5_000,
			timeoutMsg: 'Sender did not settle at bottom after filling',
		});
	});

	it('returns to bottom when the user sends while scrolled up', async () => {
		await agent1.scrollChatUp();
		expect(await agent1.isScrollAtBottom()).toBe(false);

		await agent1.sendMessage('self-send after scroll up');
		await agent1.waitForMessage('self-send after scroll up');

		await agent1.waitUntil(async () => agent1.isScrollAtBottom(), {
			timeout: 5_000,
			timeoutMsg: 'Did not return to bottom after self-send',
		});
	});

	it('stays pinned to bottom when a peer message arrives at bottom', async () => {
		expect(await agent1.isScrollAtBottom()).toBe(true);

		await agent2.sendMessage('peer at bottom');
		await agent1.waitForMessage('peer at bottom');

		expect(await agent1.isScrollAtBottom()).toBe(true);
	});

	it('does not auto-scroll when a peer message arrives while scrolled up', async () => {
		await agent1.scrollChatUp();
		expect(await agent1.isScrollAtBottom()).toBe(false);

		await agent2.sendMessage('peer while scrolled up');
		await agent1.waitForMessage('peer while scrolled up');

		expect(await agent1.isScrollAtBottom()).toBe(false);
		expect(await agent1.scrollBottomButtonVisible()).toBe(true);
		expect(await agent1.unreadBadgeText()).toBeTruthy();
	});

	it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
		// Establish the precondition (scrolled up + unread badge visible)
		// from scratch rather than depending on the previous test, so this
		// test survives Mocha bail/retry and reordering.
		await agent1.scrollChatUp();
		await agent2.sendMessage('unread badge precondition');
		await agent1.waitForMessage('unread badge precondition');
		await agent1.waitUntil(
			async () => (await agent1.unreadBadgeText()) !== null,
			{
				timeout: 5_000,
				timeoutMsg: 'Unread badge did not appear after peer message',
			},
		);
		expect(await agent1.scrollBottomButtonVisible()).toBe(true);

		await agent1.clickScrollBottomButton();

		await agent1.waitUntil(async () => agent1.isScrollAtBottom(), {
			timeout: 5_000,
			timeoutMsg: 'Did not return to bottom after clicking the button',
		});
		await agent1.waitUntil(
			async () => (await agent1.unreadBadgeText()) === null,
			{
				timeout: 5_000,
				timeoutMsg: 'Unread badge did not clear after returning to bottom',
			},
		);
	});

	it('hides the scroll-to-bottom button once the user scrolls back down', async () => {
		await agent1.scrollChatUp();
		expect(await agent1.scrollBottomButtonVisible()).toBe(true);

		await agent1.scrollChatToBottom();
		await agent1.waitUntil(
			async () => !(await agent1.scrollBottomButtonVisible()),
			{
				timeout: 5_000,
				timeoutMsg: 'Scroll-to-bottom button still visible at bottom',
			},
		);
	});

	// Guards against silent regressions in the Konsta selector inside
	// chat-scroll.ts. The transparent navbar's bg should be opaque
	// (opacity '1') at the bottom — where the latest message sits right
	// under the navbar — and fade out ('0') only once the user scrolls
	// all the way to the welcome / avatar surface at the top of the chat.
	it('toggles transparent navbar opacity on scroll', async () => {
		expect(await agent1.isScrollAtBottom()).toBe(true);
		await agent1.waitUntil(
			async () => (await agent1.navbarBgOpacity()) === '1',
			{
				timeout: 5_000,
				timeoutMsg: 'Navbar opacity not 1 at bottom',
			},
		);

		await agent1.scrollChatToTop();
		await agent1.waitUntil(
			async () => (await agent1.navbarBgOpacity()) === '0',
			{
				timeout: 5_000,
				timeoutMsg:
					'Navbar opacity did not flip to 0 at the top of the chat',
			},
		);

		await agent1.scrollChatToBottom();
		await agent1.waitUntil(
			async () => (await agent1.navbarBgOpacity()) === '1',
			{
				timeout: 5_000,
				timeoutMsg: 'Navbar opacity did not flip back to 1 at bottom',
			},
		);
	});
});
