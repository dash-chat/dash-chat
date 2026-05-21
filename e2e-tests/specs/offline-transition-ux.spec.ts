/**
 * Offline UX E2E.
 *
 * Drives the per-message status indicator and the chat navbar connection chip
 * through every visible state by suspending the global cloud mailbox and
 * toggling the per-agent local mailbox server underneath the running agents.
 *
 * Visible states under test:
 *   - Message status: "cloud" → "sending" → "local" → "cloud"
 *   - Navbar chip:    hidden (connected) → disconnected → local → hidden (connected)
 */

import {
	type Agent,
	exchangeContacts,
	setupAgent,
} from '../helpers/setup-agents';
import { resumeMailbox, suspendMailbox } from '../helpers/mailbox-control';

describe('Offline UX', () => {
	let agent1: Agent;
	let agent2: Agent;
	let mailboxSuspended = false;

	before(async function () {
		this.timeout(120_000);
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfile('Alice', 'Test');
		await agent2.createProfile('Bob', 'Test');
		// exchangeContacts leaves agent1 inside its direct chat with Bob —
		// that's where MessageStatusIndicator and ConnectionStatusIndicator
		// are mounted.
		await exchangeContacts(agent1, agent2);
	});

	after(() => {
		// Safety net: if a test bailed mid-suspend, undo the SIGSTOP so we
		// don't strand the mailbox process for the rest of the run.
		if (mailboxSuspended) {
			try {
				resumeMailbox();
			} catch {
				/* ignore */
			}
			mailboxSuspended = false;
		}
	});

	describe('cloud mailbox online', () => {
		it('sends a message, peer receives it, sender shows the cloud check, and the navbar chip stays hidden', async () => {
			await agent1.sendMessage('online hello');
			await agent2.waitForMessage('online hello');

			await agent1.waitUntil(
				async () => (await agent1.lastMessageStatus()) === 'cloud',
				{
					timeout: 15_000,
					timeoutMsg:
						'Last message did not advance to "cloud" while mailbox was up',
				},
			);
			expect(await agent1.connectionStatus()).toBe('connected');
		});
	});

	describe('cloud mailbox stopped', () => {
		before(() => {
			suspendMailbox();
			mailboxSuspended = true;
		});

		after(() => {
			if (mailboxSuspended) {
				resumeMailbox();
				mailboxSuspended = false;
			}
		});

		it('new messages stay on the sending spinner', async () => {
			await agent1.sendMessage('offline hello');
			// Spinner shows up immediately and persists — we picked a long
			// enough hold (5s > active_interval) to catch a spurious flip,
			// without slowing the suite down.
			await agent1.waitUntil(
				async () => (await agent1.lastMessageStatus()) === 'sending',
				{ timeout: 5_000, timeoutMsg: 'Message never entered "sending"' },
			);
			await new Promise(r => setTimeout(r, 5_000));
			expect(await agent1.lastMessageStatus()).toBe('sending');
		});

		// connect_timeout=5s + timeout=10s × degraded_threshold(5) gives a
		// worst case around ~60s before the chip flips. Pad on top so a slow
		// runner doesn't false-fail.
		it('navbar chip flips to "disconnected" once consecutive errors accumulate', async () => {
			await agent1.waitUntil(
				async () => (await agent1.connectionStatus()) === 'disconnected',
				{
					timeout: 90_000,
					interval: 1_000,
					timeoutMsg: 'Navbar chip never flipped to "disconnected"',
				},
			);
		});

		describe('with local mailbox enabled on the sender', () => {
			before(async function () {
				this.timeout(60_000);
				// Spawns the local mailbox-server, advertises it via mDNS; the
				// agent's own discovery loop registers it as a mailbox client.
				await agent1.setLocalMailboxEnabled(true);
			});

			after(async () => {
				await agent1.setLocalMailboxEnabled(false);
			});

			it('navbar chip switches from "disconnected" to "local" once the local mailbox is discovered and polled', async () => {
				await agent1.waitUntil(
					async () => (await agent1.connectionStatus()) === 'local',
					{
						timeout: 60_000,
						interval: 500,
						timeoutMsg:
							'Navbar chip never flipped to "local" after enabling local mailbox',
					},
				);
			});

			it('a new message advances to the "local" mailbox icon', async () => {
				await agent1.sendMessage('local hello');
				await agent1.waitUntil(
					async () => (await agent1.lastMessageStatus()) === 'local',
					{
						timeout: 30_000,
						interval: 500,
						timeoutMsg:
							'New message did not reach the "local" sync state',
					},
				);
			});
		});
	});

	describe('cloud mailbox back online', () => {
		it('navbar chip hides again and pending message advances to "cloud"', async () => {
			await agent1.waitUntil(
				async () => (await agent1.connectionStatus()) === 'connected',
				{
					timeout: 30_000,
					interval: 500,
					timeoutMsg: 'Navbar chip did not return to "connected"',
				},
			);
			await agent1.waitUntil(
				async () => (await agent1.lastMessageStatus()) === 'cloud',
				{
					timeout: 30_000,
					timeoutMsg:
						'Previously-pending message did not advance to "cloud" after mailbox came back',
				},
			);
			await agent2.waitForMessage('offline hello');
		});
	});
});
