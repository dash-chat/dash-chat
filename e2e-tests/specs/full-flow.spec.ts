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
	waitForMessage,
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
});
