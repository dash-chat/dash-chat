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
	scrollChatUp,
	scrollBottomButtonVisible,
	unreadBadgeText,
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
		// Enough messages to overflow the viewport regardless of screen size.
		const FILLER_COUNT = 25;

		it(`fills the chat with ${FILLER_COUNT} messages so it overflows`, async () => {
			const agent1 = browser.getInstance('agent1');
			for (let i = 0; i < FILLER_COUNT; i++) {
				await sendMessage(agent1, `filler ${i}`);
			}
			await waitForMessage(agent1, `filler ${FILLER_COUNT - 1}`, 30_000);
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
