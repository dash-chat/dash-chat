/**
 * Backwards compatibility test — Phase 1: Setup
 *
 * Runs against the OLD version binary. Creates profiles, exchanges contacts,
 * and sends messages. Saves test state to .dbs/compat/state.json so the
 * verify phase can check everything persisted correctly.
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

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
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('creates profiles on both agents', async () => {
		await agent1.createProfilePage.createProfile(ALICE_NAME, ALICE_SURNAME);
		await agent2.createProfilePage.createProfile(BOB_NAME, BOB_SURNAME);
	});

	it('exchanges contact codes between agents', async () => {
		await exchangeContacts(agent1, agent2);
	});

	it('sends a message from Alice to Bob', async () => {
		await agent1.directChatPage.sendMessage(MSG_ALICE);
		await agent2.directChatPage.messages.waitForMessage(MSG_ALICE);
	});

	it('sends a reply from Bob to Alice', async () => {
		await agent2.directChatPage.sendMessage(MSG_BOB);
		await agent1.directChatPage.messages.waitForMessage(MSG_BOB);
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
