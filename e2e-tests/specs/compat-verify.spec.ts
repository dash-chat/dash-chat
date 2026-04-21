/**
 * Backwards compatibility test — Phase 2: Verify
 *
 * Runs against the CURRENT version binary using data created by the old
 * version in the setup phase. Verifies that profiles, contacts, and messages
 * all persisted correctly, and that new messages can be sent.
 */

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	waitForBothAgents,
	openDirectChat,
	sendMessage,
	waitForMessage,
} from '../helpers/setup-agents';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const STATE_FILE = path.join(ROOT, '.dbs', 'compat', 'state.json');

interface CompatState {
	aliceName: string;
	aliceSurname: string;
	bobName: string;
	bobSurname: string;
	msgAlice: string;
	msgBob: string;
}

let state: CompatState;

const NEW_MSG_ALICE = 'Post-upgrade message from Alice!';
const NEW_MSG_BOB = 'Post-upgrade message from Bob!';

describe('Compat verify — check data with current version', () => {
	before(async () => {
		if (!existsSync(STATE_FILE)) {
			throw new Error(`State file not found: ${STATE_FILE}. Did the setup phase run?`);
		}
		state = JSON.parse(readFileSync(STATE_FILE, 'utf-8'));

		await waitForBothAgents();
	});

	it('both agents skip profile creation (profiles persisted)', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// If profiles persisted, the app goes straight to the home screen.
		// Wait for a home-screen element (chat list or empty state) to appear.
		const err1 = await agent1.executeAsync((done: (r: string | null) => void) => {
			window.__test
				.waitFor('[data-testid="all-chats-list"], [data-testid="all-chats-empty"]', 10_000)
				.then(() => done(null), (e) => done(String(e)));
		});
		expect(err1).toBeNull();

		const err2 = await agent2.executeAsync((done: (r: string | null) => void) => {
			window.__test
				.waitFor('[data-testid="all-chats-list"], [data-testid="all-chats-empty"]', 10_000)
				.then(() => done(null), (e) => done(String(e)));
		});
		expect(err2).toBeNull();
	});

	it('contact names are visible in the chat list', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await agent1.waitUntil(
			async () => {
				const text = await agent1.execute(() => document.body.innerText);
				return (text as string).includes(state.bobName);
			},
			{ timeout: 15_000, interval: 1000, timeoutMsg: `Alice never saw "${state.bobName}" in chat list` },
		);

		await agent2.waitUntil(
			async () => {
				const text = await agent2.execute(() => document.body.innerText);
				return (text as string).includes(state.aliceName);
			},
			{ timeout: 15_000, interval: 1000, timeoutMsg: `Bob never saw "${state.aliceName}" in chat list` },
		);
	});

	it('old messages are still visible after upgrade', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await openDirectChat(agent1, state.bobName);
		await waitForMessage(agent1, state.msgAlice);
		await waitForMessage(agent1, state.msgBob);

		await openDirectChat(agent2, state.aliceName);
		await waitForMessage(agent2, state.msgAlice);
		await waitForMessage(agent2, state.msgBob);
	});

	it('can send new messages after upgrade', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await openDirectChat(agent1, state.bobName);
		await openDirectChat(agent2, state.aliceName);

		// Alice sends a new message
		await sendMessage(agent1, NEW_MSG_ALICE);
		await waitForMessage(agent2, NEW_MSG_ALICE);

		// Bob sends a new message
		await sendMessage(agent2, NEW_MSG_BOB);
		await waitForMessage(agent1, NEW_MSG_BOB);
	});
});
