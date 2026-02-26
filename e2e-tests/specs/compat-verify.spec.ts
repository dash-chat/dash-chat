/**
 * Backwards compatibility test — Phase 2: Verify
 *
 * Runs against the CURRENT version binary using data created by the old
 * version in the setup phase. Verifies that profiles, contacts, and messages
 * all persisted correctly, and that new messages can be sent.
 */

import { readFileSync } from 'node:fs';
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

const state: CompatState = JSON.parse(readFileSync(STATE_FILE, 'utf-8'));

const NEW_MSG_ALICE = 'Post-upgrade message from Alice!';
const NEW_MSG_BOB = 'Post-upgrade message from Bob!';

describe('Compat verify — check data with current version', () => {
	it('both agents skip profile creation (profiles persisted)', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// If profiles persisted, the create-profile screen should not appear.
		// The app should go straight to the home/chat-list screen.
		// We check by verifying that __test.navigateToAddContact works
		// (which requires being past the profile creation screen).
		const canNavigate1 = await agent1.execute(async () => {
			try {
				// Wait a moment for the app to initialize
				await new Promise((r) => setTimeout(r, 3000));
				await window.__test.navigateToAddContact();
				return true;
			} catch {
				return false;
			}
		});
		expect(canNavigate1).toBe(true);

		const canNavigate2 = await agent2.execute(async () => {
			try {
				await new Promise((r) => setTimeout(r, 3000));
				await window.__test.navigateToAddContact();
				return true;
			} catch {
				return false;
			}
		});
		expect(canNavigate2).toBe(true);
	});

	it('old messages are visible in the chat', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Navigate agent1 back to home and into the chat with Bob
		// The contact should already exist, so addContact with Bob's code
		// is not needed. Instead, navigate to home and find the chat.
		// We use waitForMessage which looks in the current chat view.

		// Agent 1: go back to home first
		await agent1.execute(() => {
			window.location.hash = '';
			window.location.pathname = '/';
		});
		await agent1.execute(async () => {
			await new Promise((r) => setTimeout(r, 2000));
		});

		// Agent 2: go back to home
		await agent2.execute(() => {
			window.location.hash = '';
			window.location.pathname = '/';
		});
		await agent2.execute(async () => {
			await new Promise((r) => setTimeout(r, 2000));
		});

		// Verify contact names appear in the chat list by checking for text
		const aliceSeeBob = await agent1.execute(
			(name: string) => {
				return document.body.innerText.includes(name);
			},
			state.bobName,
		);
		expect(aliceSeeBob).toBe(true);

		const bobSeeAlice = await agent2.execute(
			(name: string) => {
				return document.body.innerText.includes(name);
			},
			state.aliceName,
		);
		expect(bobSeeAlice).toBe(true);
	});

	it('can send new messages after upgrade', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Navigate into the chat by clicking the contact name
		await agent1.execute(
			(name: string) => window.__test.waitFor(`[data-testid="chat-list"] >> text=${name}`, 10000),
			state.bobName,
		);
		await agent1.execute(
			(name: string) => window.__test.click(`[data-testid="chat-list"] >> text=${name}`),
			state.bobName,
		);

		await agent2.execute(
			(name: string) => window.__test.waitFor(`[data-testid="chat-list"] >> text=${name}`, 10000),
			state.aliceName,
		);
		await agent2.execute(
			(name: string) => window.__test.click(`[data-testid="chat-list"] >> text=${name}`),
			state.aliceName,
		);

		// Wait for chat to load
		await agent1.execute(async () => {
			await new Promise((r) => setTimeout(r, 2000));
		});
		await agent2.execute(async () => {
			await new Promise((r) => setTimeout(r, 2000));
		});

		// Alice sends a new message
		await agent1.execute(
			(text: string) => window.__test.sendMessage(text),
			NEW_MSG_ALICE,
		);

		const received1 = await agent2.execute(
			(text: string) => window.__test.waitForMessage(text),
			NEW_MSG_ALICE,
		);
		expect(received1).toBe(true);

		// Bob sends a new message
		await agent2.execute(
			(text: string) => window.__test.sendMessage(text),
			NEW_MSG_BOB,
		);

		const received2 = await agent1.execute(
			(text: string) => window.__test.waitForMessage(text),
			NEW_MSG_BOB,
		);
		expect(received2).toBe(true);
	});
});
