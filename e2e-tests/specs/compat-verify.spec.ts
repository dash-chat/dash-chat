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

import { type Agent, setupAgents } from '../setup/setup-agents';

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
let agent1: Agent;
let agent2: Agent;

const NEW_MSG_ALICE = 'Post-upgrade message from Alice!';
const NEW_MSG_BOB = 'Post-upgrade message from Bob!';

describe('Compat verify — check data with current version', () => {
	before(async function () {
		if (!existsSync(STATE_FILE)) {
			throw new Error(
				`State file not found: ${STATE_FILE}. Did the setup phase run?`,
			);
		}
		state = JSON.parse(readFileSync(STATE_FILE, 'utf-8'));

		[agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
	});

	it('both agents skip profile creation (profiles persisted)', async () => {
		await agent1.homePage.ready();
		await agent2.homePage.ready();
	});

	it('contact names are visible in the chat list', async () => {
		await agent1.waitUntil(async () =>
			agent1.homePage.hasChatListItem(state.bobName),
		);
		await agent2.waitUntil(async () =>
			agent2.homePage.hasChatListItem(state.aliceName),
		);
	});

	it('old messages are still visible after upgrade', async () => {
		await agent1.homePage.openChat(state.bobName);
		await agent1.directChatPage.messages.waitForMessage(state.msgAlice);
		await agent1.directChatPage.messages.waitForMessage(state.msgBob);

		await agent2.homePage.openChat(state.aliceName);
		await agent2.directChatPage.messages.waitForMessage(state.msgAlice);
		await agent2.directChatPage.messages.waitForMessage(state.msgBob);
	});

	it('can send new messages after upgrade', async () => {
		await agent1.directChatPage.sendMessage(NEW_MSG_ALICE);
		await agent2.directChatPage.messages.waitForMessage(NEW_MSG_ALICE);

		await agent2.directChatPage.sendMessage(NEW_MSG_BOB);
		await agent1.directChatPage.messages.waitForMessage(NEW_MSG_BOB);
	});
});
