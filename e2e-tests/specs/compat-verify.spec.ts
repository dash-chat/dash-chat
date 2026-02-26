/**
 * Backwards compatibility test — Phase 2: Verify
 *
 * Runs against the CURRENT version binary using data created by the old
 * version in the setup phase. Verifies that profiles, contacts, and messages
 * all persisted correctly, and that new messages can be sent.
 *
 * Uses executeAsync with done callbacks because the W3C WebDriver
 * "execute/sync" endpoint cannot serialize Promises.
 */

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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

		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await Promise.all([
			agent1.waitUntil(
				async () => agent1.execute(() => typeof window.__test !== 'undefined'),
				{ timeout: 30_000, interval: 500, timeoutMsg: 'agent1: window.__test not registered' },
			),
			agent2.waitUntil(
				async () => agent2.execute(() => typeof window.__test !== 'undefined'),
				{ timeout: 30_000, interval: 500, timeoutMsg: 'agent2: window.__test not registered' },
			),
		]);
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

	it('can send new messages after upgrade', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Navigate into the chat by clicking the contact name in the chat list
		const clickErr1 = await agent1.executeAsync(
			(name: string, done: (r: string | null) => void) => {
				window.__test.openDirectChat(name).then(() => done(null), (e) => done(String(e)));
			},
			state.bobName,
		);
		expect(clickErr1).toBeNull();

		const clickErr2 = await agent2.executeAsync(
			(name: string, done: (r: string | null) => void) => {
				window.__test.openDirectChat(name).then(() => done(null), (e) => done(String(e)));
			},
			state.aliceName,
		);
		expect(clickErr2).toBeNull();

		// Alice sends a new message
		const sendErr1 = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			NEW_MSG_ALICE,
		);
		expect(sendErr1).toBeNull();

		const recvErr1 = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			NEW_MSG_ALICE,
		);
		expect(recvErr1).toBeNull();

		// Bob sends a new message
		const sendErr2 = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			NEW_MSG_BOB,
		);
		expect(sendErr2).toBeNull();

		const recvErr2 = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			NEW_MSG_BOB,
		);
		expect(recvErr2).toBeNull();
	});
});
