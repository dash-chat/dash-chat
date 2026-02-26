/**
 * Backwards compatibility test — Phase 1: Setup
 *
 * Runs against the OLD version binary. Creates profiles, exchanges contacts,
 * and sends messages. Saves test state to .dbs/compat/state.json so the
 * verify phase can check everything persisted correctly.
 *
 * Uses executeAsync with done callbacks because the W3C WebDriver
 * "execute/sync" endpoint cannot serialize Promises.
 */

import { writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const STATE_FILE = path.join(ROOT, '.dbs', 'compat', 'state.json');

const ALICE_NAME = 'Alice';
const ALICE_SURNAME = 'Compat';
const BOB_NAME = 'Bob';
const BOB_SURNAME = 'Compat';
const MSG_ALICE = 'Hello from old Alice!';
const MSG_BOB = 'Hello from old Bob!';

describe('Compat setup — create data with old version', () => {
	before(async () => {
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

	it('creates profiles on both agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const err1 = await agent1.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			ALICE_NAME,
			ALICE_SURNAME,
		);
		expect(err1).toBeNull();

		const err2 = await agent2.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			BOB_NAME,
			BOB_SURNAME,
		);
		expect(err2).toBeNull();
	});

	it('exchanges contact codes between agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const navErr1 = await agent1.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		expect(navErr1).toBeNull();

		const aliceCode = await agent1.execute(() => window.__test.getContactCode());
		expect(aliceCode).toBeTruthy();

		const navErr2 = await agent2.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		expect(navErr2).toBeNull();

		const bobCode = await agent2.execute(() => window.__test.getContactCode());
		expect(bobCode).toBeTruthy();

		const addErr1 = await agent1.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			bobCode as string,
		);
		expect(addErr1).toBeNull();

		const addErr2 = await agent2.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			aliceCode as string,
		);
		expect(addErr2).toBeNull();
	});

	it('sends a message from Alice to Bob', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const sendErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			MSG_ALICE,
		);
		expect(sendErr).toBeNull();

		const recvErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			MSG_ALICE,
		);
		expect(recvErr).toBeNull();
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const sendErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			MSG_BOB,
		);
		expect(sendErr).toBeNull();

		const recvErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			MSG_BOB,
		);
		expect(recvErr).toBeNull();
	});

	it('saves test state for verify phase', () => {
		const state = {
			aliceName: ALICE_NAME,
			aliceSurname: ALICE_SURNAME,
			bobName: BOB_NAME,
			bobSurname: BOB_SURNAME,
			msgAlice: MSG_ALICE,
			msgBob: MSG_BOB,
		};
		mkdirSync(path.dirname(STATE_FILE), { recursive: true });
		writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
	});
});
