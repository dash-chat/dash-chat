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
	chatOverflow,
	clickScrollBottomButton,
	createProfile,
	exchangeContacts,
	isScrollAtBottom,
	navbarBgOpacity,
	scrollBottomButtonVisible,
	scrollChatToBottom,
	scrollChatToTop,
	scrollChatUp,
	sendMessage,
	unreadBadgeText,
	waitForBothAgents,
	waitForMessage,
} from '../helpers/setup-agents';

describe('Chat scroll behavior', () => {
	// Need enough overflow that scrollChatUp can move past the bottom
	// threshold (200px) — leave headroom so timing/layout jitter doesn't
	// drop us below.
	const REQUIRED_OVERFLOW = 400;
	// Hard cap so a misconfigured viewport can't loop forever.
	const MAX_FILLER = 200;

	before(async function () {
		this.timeout(120_000);
		await waitForBothAgents();
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await createProfile(agent1, 'Alice', 'Test');
		await createProfile(agent2, 'Bob', 'Test');
		// addContact (inside exchangeContacts) lands agent1 on the direct chat
		// with Bob, which is exactly where the scroll tests need to start.
		await exchangeContacts(agent1, agent2);
	});

	it('fills the chat until it overflows enough to scroll', async () => {
		const agent1 = browser.getInstance('agent1');
		let i = 0;
		let overflow = await chatOverflow(agent1);
		while (overflow < REQUIRED_OVERFLOW && i < MAX_FILLER) {
			await sendMessage(agent1, `filler ${i}`);
			// Wait for the message to render before measuring — DOM updates
			// are async after the click.
			await waitForMessage(agent1, `filler ${i}`, 10_000);
			overflow = await chatOverflow(agent1);
			i++;
		}
		expect(overflow).toBeGreaterThanOrEqual(REQUIRED_OVERFLOW);
		await agent1.waitUntil(async () => isScrollAtBottom(agent1), {
			timeout: 5_000,
			timeoutMsg: 'Sender did not settle at bottom after filling',
		});
	});

	it('returns to bottom when the user sends while scrolled up', async () => {
		const agent1 = browser.getInstance('agent1');
		await scrollChatUp(agent1);
		expect(await isScrollAtBottom(agent1)).toBe(false);

		await sendMessage(agent1, 'self-send after scroll up');
		await waitForMessage(agent1, 'self-send after scroll up', 30_000);

		await agent1.waitUntil(async () => isScrollAtBottom(agent1), {
			timeout: 5_000,
			timeoutMsg: 'Did not return to bottom after self-send',
		});
	});

	it('stays pinned to bottom when a peer message arrives at bottom', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		expect(await isScrollAtBottom(agent1)).toBe(true);

		await sendMessage(agent2, 'peer at bottom');
		await waitForMessage(agent1, 'peer at bottom');

		expect(await isScrollAtBottom(agent1)).toBe(true);
	});

	it('does not auto-scroll when a peer message arrives while scrolled up', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await scrollChatUp(agent1);
		expect(await isScrollAtBottom(agent1)).toBe(false);

		await sendMessage(agent2, 'peer while scrolled up');
		await waitForMessage(agent1, 'peer while scrolled up');

		expect(await isScrollAtBottom(agent1)).toBe(false);
		expect(await scrollBottomButtonVisible(agent1)).toBe(true);
		expect(await unreadBadgeText(agent1)).toBeTruthy();
	});

	it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Establish the precondition (scrolled up + unread badge visible)
		// from scratch rather than depending on the previous test, so this
		// test survives Mocha bail/retry and reordering.
		await scrollChatUp(agent1);
		await sendMessage(agent2, 'unread badge precondition');
		await waitForMessage(agent1, 'unread badge precondition');
		await agent1.waitUntil(
			async () => (await unreadBadgeText(agent1)) !== null,
			{
				timeout: 5_000,
				timeoutMsg: 'Unread badge did not appear after peer message',
			},
		);
		expect(await scrollBottomButtonVisible(agent1)).toBe(true);

		await clickScrollBottomButton(agent1);

		await agent1.waitUntil(async () => isScrollAtBottom(agent1), {
			timeout: 5_000,
			timeoutMsg: 'Did not return to bottom after clicking the button',
		});
		await agent1.waitUntil(
			async () => (await unreadBadgeText(agent1)) === null,
			{
				timeout: 5_000,
				timeoutMsg: 'Unread badge did not clear after returning to bottom',
			},
		);
	});

	it('hides the scroll-to-bottom button once the user scrolls back down', async () => {
		const agent1 = browser.getInstance('agent1');

		await scrollChatUp(agent1);
		expect(await scrollBottomButtonVisible(agent1)).toBe(true);

		await scrollChatToBottom(agent1);
		await agent1.waitUntil(
			async () => !(await scrollBottomButtonVisible(agent1)),
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
		const agent1 = browser.getInstance('agent1');
		expect(await isScrollAtBottom(agent1)).toBe(true);
		await agent1.waitUntil(
			async () => (await navbarBgOpacity(agent1)) === '1',
			{
				timeout: 5_000,
				timeoutMsg: 'Navbar opacity not 1 at bottom',
			},
		);

		await scrollChatToTop(agent1);
		await agent1.waitUntil(
			async () => (await navbarBgOpacity(agent1)) === '0',
			{
				timeout: 5_000,
				timeoutMsg:
					'Navbar opacity did not flip to 0 at the top of the chat',
			},
		);

		await scrollChatToBottom(agent1);
		await agent1.waitUntil(
			async () => (await navbarBgOpacity(agent1)) === '1',
			{
				timeout: 5_000,
				timeoutMsg: 'Navbar opacity did not flip back to 1 at bottom',
			},
		);
	});
});
