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
import { type Agent, setupAgent } from '../helpers/setup-agents';

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
	before(async () => {
		if (!existsSync(STATE_FILE)) {
			throw new Error(`State file not found: ${STATE_FILE}. Did the setup phase run?`);
		}
		state = JSON.parse(readFileSync(STATE_FILE, 'utf-8'));

		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('both agents skip profile creation (profiles persisted)', async () => {
		// If profiles persisted, the app goes straight to the home screen.
		await agent1.waitUntil(async () => !!(await agent1.homeLoaded()), {
			timeout: 10_000,
			timeoutMsg: 'Alice did not reach the home screen',
		});
		await agent2.waitUntil(async () => !!(await agent2.homeLoaded()), {
			timeout: 10_000,
			timeoutMsg: 'Bob did not reach the home screen',
		});
	});

	it('contact names are visible in the chat list', async () => {
		await agent1.waitUntil(
			async () => (await agent1.getChatListItem(state.bobName)) !== null,
			{ timeout: 15_000, interval: 1000, timeoutMsg: `Alice never saw "${state.bobName}" in chat list` },
		);

		await agent2.waitUntil(
			async () => (await agent2.getChatListItem(state.aliceName)) !== null,
			{ timeout: 15_000, interval: 1000, timeoutMsg: `Bob never saw "${state.aliceName}" in chat list` },
		);
	});

	it('old messages are still visible after upgrade', async () => {
		await agent1.openDirectChat(state.bobName);
		await agent1.waitForMessage(state.msgAlice);
		await agent1.waitForMessage(state.msgBob);

		await agent2.openDirectChat(state.aliceName);
		await agent2.waitForMessage(state.msgAlice);
		await agent2.waitForMessage(state.msgBob);
	});

	it('can send new messages after upgrade', async () => {
		await agent1.openDirectChat(state.bobName);
		await agent2.openDirectChat(state.aliceName);

		// Alice sends a new message
		await agent1.sendMessage(NEW_MSG_ALICE);
		await agent2.waitForMessage(NEW_MSG_ALICE);

		// Bob sends a new message
		await agent2.sendMessage(NEW_MSG_BOB);
		await agent1.waitForMessage(NEW_MSG_BOB);
	});
});
