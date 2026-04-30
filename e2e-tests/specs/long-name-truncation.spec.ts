/**
 * Long-name truncation E2E test.
 *
 * Verifies that contacts with very long names don't cause horizontal overflow
 * in the chat list or the direct-chat navbar.
 */

import {
	waitForBothAgents,
	createProfile,
	exchangeContacts,
	openDirectChat,
} from '../helpers/setup-agents';

const LONG_NAME = 'Bartholomew';
const LONG_SURNAME = 'Wolfeschlegelsteinhausenbergerdorff';

describe('Long name truncation', () => {
	before(async () => {
		await waitForBothAgents();
	});

	it('creates profiles — agent1 with a very long name', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await createProfile(agent1, LONG_NAME, LONG_SURNAME);
		await createProfile(agent2, 'Bob', 'Test');
	});

	it('exchanges contacts', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await exchangeContacts(agent1, agent2);
	});

	it('chat list shows long name without horizontal overflow', async () => {
		const agent2 = browser.getInstance('agent2');

		// Wait for the chat entry to appear in agent2's chat list
		await agent2.waitUntil(
			async () =>
				agent2.execute(
					(name: string) =>
						!!document.querySelector('[data-testid="all-chats-list"]')?.textContent?.includes(name),
					LONG_NAME,
				),
			{ timeout: 30_000, interval: 1_000, timeoutMsg: 'Long-name contact not in chat list' },
		);

		const overflowIssues = await agent2.execute(() =>
			window.__test.checkChatListOverflow(),
		) as string[];

		expect(overflowIssues).toEqual([]);
	});

	it('direct-chat navbar shows long name without horizontal overflow', async () => {
		const agent2 = browser.getInstance('agent2');
		await openDirectChat(agent2, LONG_NAME);

		// Peer name element should be present
		const peerNamePresent = await agent2.execute(() =>
			!!document.querySelector('[data-testid="direct-chat-peer-name"]'),
		);
		expect(peerNamePresent).toBe(true);

		// Navbar should not overflow
		const navbarOverflow = await agent2.execute(() => {
			const navbar = document.querySelector('.k-navbar');
			if (!navbar) return ['Navbar element not found'];
			const issues: string[] = [];
			if (navbar.scrollWidth > navbar.clientWidth + 2) {
				issues.push('Navbar has horizontal overflow');
			}
			navbar.querySelectorAll('*').forEach((el) => {
				if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
					const text = el.textContent?.substring(0, 60).trim();
					if (text) issues.push(`Overflow in navbar <${el.tagName.toLowerCase()}>: "${text}"`);
				}
			});
			return issues.slice(0, 10);
		}) as string[];

		expect(navbarOverflow).toEqual([]);
	});
});
