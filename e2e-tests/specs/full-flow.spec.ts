/**
 * Full messaging flow E2E test.
 *
 * Uses two Tauri instances (agent1 & agent2) via WebdriverIO multiremote.
 * Calls window.__test functions registered by ui/tests/setup-utils.ts.
 */

import {
	waitForBothAgents,
	createProfile,
	getStartedCards,
	dismissGetStartedCard,
	waitForTestUtils,
	exchangeContacts,
	sendMessage,
	waitForMessage,
	isScrollAtBottom,
	chatOverflow,
	scrollChatUp,
	scrollBottomButtonVisible,
	unreadBadgeText,
	clickScrollBottomButton,
	scrollChatToBottom,
	scrollChatToTop,
	navbarBgOpacity,
	openDirectChat,
} from '../helpers/setup-agents';

describe('Full messaging flow', () => {
	before(async () => {
		await waitForBothAgents();
	});

	it('creates profiles on both agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await createProfile(agent1, 'Alice', 'Test');
		await createProfile(agent2, 'Bob', 'Test');
	});

	it('shows Get Started cards on empty home', async () => {
		const agent1 = browser.getInstance('agent1');

		await agent1.waitUntil(
			async () => (await getStartedCards(agent1)).length > 0,
			{ timeout: 10_000, timeoutMsg: 'No Get Started cards visible' },
		);

		const cards = await getStartedCards(agent1);
		expect(cards).toContain('add-contact');
		expect(cards).toContain('add-photo');
		expect(cards).toContain('chat-color');
	});

	it('dismisses a Get Started card and it persists after reload', async () => {
		const agent1 = browser.getInstance('agent1');

		await dismissGetStartedCard(agent1, 'add-contact');

		await agent1.waitUntil(
			async () => !(await getStartedCards(agent1)).includes('add-contact'),
			{ timeout: 5_000, timeoutMsg: 'Add contact card still visible after dismiss' },
		);

		// Reload and verify dismissal persists
		await agent1.execute(() => window.location.reload());
		await waitForTestUtils(agent1);
		await agent1.waitUntil(
			async () => agent1.execute(() => window.__test.homeLoaded() !== null),
			{ timeout: 10_000, timeoutMsg: 'Home page not loaded after reload' },
		);

		const cards = await getStartedCards(agent1);
		expect(cards).not.toContain('add-contact');
		expect(cards).toContain('add-photo');
	});

	it('exchanges contact codes between agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a message from Alice to Bob', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await sendMessage(agent1, 'Hello from Alice!');

		// Verify message appears on sender (should be near-instant)
		await waitForMessage(agent1, 'Hello from Alice!', 30_000);

		// Wait for message on receiver via mailbox sync
		await waitForMessage(agent2, 'Hello from Alice!');
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await sendMessage(agent2, 'Hello from Bob!');

		await waitForMessage(agent2, 'Hello from Bob!', 30_000);

		await waitForMessage(agent1, 'Hello from Bob!');
	});

	describe('chat scroll behavior', () => {
		// Need enough overflow that scrollChatUp can move past the bottom
		// threshold (200px) — leave headroom so timing/layout jitter doesn't
		// drop us below.
		const REQUIRED_OVERFLOW = 400;
		// Hard cap so a misconfigured viewport can't loop forever.
		const MAX_FILLER = 200;

		// Land agent1 on the direct chat with Bob regardless of where prior
		// tests left it. Makes this suite independent of test ordering.
		before(async () => {
			const agent1 = browser.getInstance('agent1');
			await agent1.execute(() => window.__test.goto('/'));
			await openDirectChat(agent1, 'Bob');
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

		// Continues from the previous test: agent1 is scrolled up with the
		// scroll-to-bottom button + unread badge visible.
		it('clicking scroll-to-bottom returns to bottom and clears unread badge', async () => {
			const agent1 = browser.getInstance('agent1');
			expect(await scrollBottomButtonVisible(agent1)).toBe(true);
			expect(await unreadBadgeText(agent1)).toBeTruthy();

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

	it('displays the app version on the help page', async () => {
		const agent1 = browser.getInstance('agent1');

		await agent1.execute(() => window.__test.goto('/settings/help'));
		await agent1.waitUntil(
			async () => agent1.execute(() => window.__test.versionItem() !== null),
			{ timeout: 10_000, timeoutMsg: 'Version item not visible on help page' },
		);

		const versionText = await agent1.execute(
			() => (window.__test.versionItem() as HTMLElement)?.textContent ?? '',
		);
		expect(versionText).toMatch(/\d+\.\d+\.\d+/);
	});
});
