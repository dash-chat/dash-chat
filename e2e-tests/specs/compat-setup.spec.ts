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
import {
	waitForBothAgents,
	createProfile,
	exchangeContacts,
	sendAndReceiveMessage,
} from '../helpers/setup-agents';

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
		console.log("%%%");
		await waitForBothAgents();
	});

	it('creates profiles on both agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await createProfile(agent1, ALICE_NAME, ALICE_SURNAME);
		await createProfile(agent2, BOB_NAME, BOB_SURNAME);
	});

	it('exchanges contact codes between agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a message from Alice to Bob', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await sendAndReceiveMessage(agent1, agent2, MSG_ALICE);
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');
		await sendAndReceiveMessage(agent2, agent1, MSG_BOB);
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
