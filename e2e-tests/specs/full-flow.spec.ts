/**
 * Full messaging flow E2E test.
 *
 * Uses two Tauri instances (agent1 & agent2) via WebdriverIO multiremote.
 * Calls window.__test functions registered by ui/tests/setup-utils.ts.
 */

import {
	waitForBothAgents,
	createProfile,
	exchangeContacts,
	sendMessage,
} from '../helpers/setup-agents';

/**
 * Poll for a message to appear in the messages container.
 * Uses WDIO's waitUntil with sync execute — avoids executeAsync with
 * long-running scripts which can hang in tauri-driver.
 */
async function waitForMessageUI(agent: WebdriverIO.Browser, text: string, timeout = 60_000): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(
			(t: string) => !!document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes(t),
			text,
		),
		{ timeout, interval: 1_000, timeoutMsg: `Message "${text}" not received within ${timeout}ms` },
	);
}

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
		await waitForMessageUI(agent1, 'Hello from Alice!', 10_000);

		// Wait for message on receiver via mailbox sync (may take up to ~30s on first run)
		await waitForMessageUI(agent2, 'Hello from Alice!');
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await sendMessage(agent2, 'Hello from Bob!');

		await waitForMessageUI(agent2, 'Hello from Bob!', 10_000);

		await waitForMessageUI(agent1, 'Hello from Bob!');
	});
});
