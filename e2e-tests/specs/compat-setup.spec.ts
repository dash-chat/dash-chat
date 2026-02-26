/**
 * Backwards compatibility test — Phase 1: Setup
 *
 * Runs against the OLD version binary. Creates profiles, exchanges contacts,
 * and sends messages. Saves test state to .dbs/compat/state.json so the
 * verify phase can check everything persisted correctly.
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
	it('creates profiles on both agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const result1 = await agent1.execute(
			(name: string, surname: string) => window.__test.createProfile(name, surname),
			ALICE_NAME,
			ALICE_SURNAME,
		);
		expect(result1).toBe(true);

		const result2 = await agent2.execute(
			(name: string, surname: string) => window.__test.createProfile(name, surname),
			BOB_NAME,
			BOB_SURNAME,
		);
		expect(result2).toBe(true);
	});

	it('exchanges contact codes between agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await agent1.execute(() => window.__test.navigateToAddContact());
		const aliceCode = await agent1.execute(() => window.__test.getContactCode());
		expect(aliceCode).toBeTruthy();

		await agent2.execute(() => window.__test.navigateToAddContact());
		const bobCode = await agent2.execute(() => window.__test.getContactCode());
		expect(bobCode).toBeTruthy();

		await agent1.execute(
			(code: string) => window.__test.addContact(code),
			bobCode as string,
		);
		await agent2.execute(
			(code: string) => window.__test.addContact(code),
			aliceCode as string,
		);
	});

	it('sends a message from Alice to Bob', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await agent1.execute(
			(text: string) => window.__test.sendMessage(text),
			MSG_ALICE,
		);

		const received = await agent2.execute(
			(text: string) => window.__test.waitForMessage(text),
			MSG_ALICE,
		);
		expect(received).toBe(true);
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await agent2.execute(
			(text: string) => window.__test.sendMessage(text),
			MSG_BOB,
		);

		const received = await agent1.execute(
			(text: string) => window.__test.waitForMessage(text),
			MSG_BOB,
		);
		expect(received).toBe(true);
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
