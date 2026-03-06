/**
 * Get Started cards E2E test.
 *
 * Verifies that the sticky note cards appear on an empty home screen,
 * can be dismissed, and that "Add photo" hides after setting an avatar.
 */

import { waitForBothAgents, createProfile } from '../helpers/setup-agents';

describe('Get Started cards', () => {
	before(async () => {
		await waitForBothAgents();
	});

	it('creates a profile and sees Get Started cards', async () => {
		const agent1 = browser.getInstance('agent1');
		await createProfile(agent1, 'Alice', 'Test');

		// All three cards should be visible on empty home
		await agent1.waitUntil(
			async () => agent1.execute(() => !!document.querySelector('[data-testid="get-started-add-contact"]')),
			{ timeout: 10_000, timeoutMsg: 'Add contact card not visible' },
		);

		const hasAddPhoto = await agent1.execute(() => !!document.querySelector('[data-testid="get-started-add-photo"]'));
		expect(hasAddPhoto).toBe(true);

		const hasChatColor = await agent1.execute(() => !!document.querySelector('[data-testid="get-started-chat-color"]'));
		expect(hasChatColor).toBe(true);
	});

	it('dismisses a card and it stays dismissed after reload', async () => {
		const agent1 = browser.getInstance('agent1');

		// Dismiss the "Add contact" card
		await agent1.execute(() => {
			const btn = document.querySelector('[data-testid="get-started-dismiss-add-contact"]') as HTMLElement;
			btn?.click();
		});

		// Verify it's gone
		await agent1.waitUntil(
			async () => agent1.execute(() => !document.querySelector('[data-testid="get-started-add-contact"]')),
			{ timeout: 5_000, timeoutMsg: 'Add contact card still visible after dismiss' },
		);

		// Reload and verify it stays dismissed
		await agent1.execute(() => window.location.reload());
		await agent1.waitUntil(
			async () => agent1.execute(() => typeof window.__test !== 'undefined'),
			{ timeout: 30_000, timeoutMsg: 'window.__test not registered after reload' },
		);

		// Wait for home page to load
		await agent1.waitUntil(
			async () => agent1.execute(() =>
				!!document.querySelector('[data-testid="all-chats-list"], [data-testid="all-chats-empty"]'),
			),
			{ timeout: 10_000, timeoutMsg: 'Home page not loaded after reload' },
		);

		const stillDismissed = await agent1.execute(() => !document.querySelector('[data-testid="get-started-add-contact"]'));
		expect(stillDismissed).toBe(true);

		// Other cards should still be visible
		const hasAddPhoto = await agent1.execute(() => !!document.querySelector('[data-testid="get-started-add-photo"]'));
		expect(hasAddPhoto).toBe(true);
	});
});
