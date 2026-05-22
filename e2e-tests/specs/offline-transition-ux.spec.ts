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

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { resumeMailbox, suspendMailbox } from '../setup/mailbox-control';
import { type Agent, setupAgent } from '../setup/setup-agents';

async function openOfflineSettings(agent: Agent): Promise<void> {
	await agent.homePage.settingsLink.click();
	await agent.settingsPage.ready();
	await agent.settingsPage.offlineLink.click();
	await agent.offlinePage.ready();
}

async function returnToChat(agent: Agent, chatName: string): Promise<void> {
	if (await agent.offlinePage.back.isExisting()) {
		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
	}
	await agent.settingsPage.back.click();
	await agent.homePage.ready();
	await agent.homePage.openChat(chatName);
}

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
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		// exchangeContacts leaves agent1 inside its direct chat with Bob —
		// that's where MessageStatusIndicator and ConnectionStatusIndicator
		// are mounted.
		await exchangeContacts(agent1, agent2);
	});

	after(() => {
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
			await agent1.directChatPage.sendMessage('online hello');
			await agent2.directChatPage.waitForMessage('online hello');

			await agent1.waitUntil(
				async () => (await agent1.directChatPage.lastMessageStatus()) === 'cloud',
			);
			expect(await agent1.directChatPage.connectionStatus()).toBe('connected');
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
			await agent1.directChatPage.sendMessage('offline hello');
			await agent1.waitUntil(
				async () => (await agent1.directChatPage.lastMessageStatus()) === 'sending',
				{ timeout: 5_000 },
			);
			await agent1.pause(5_000);
			expect(await agent1.directChatPage.lastMessageStatus()).toBe('sending');
		});

		// connect_timeout=5s + timeout=10s × degraded_threshold(5) gives a
		// worst case around ~60s before the chip flips. Pad on top so a slow
		// runner doesn't false-fail.
		it('navbar chip flips to "disconnected" once consecutive errors accumulate', async () => {
			await agent1.waitUntil(
				async () => (await agent1.directChatPage.connectionStatus()) === 'disconnected',
				{ timeout: 90_000, interval: 1_000 },
			);
		});

		describe('with local mailbox enabled on the sender', () => {
			before(async function () {
				this.timeout(60_000);
				await openOfflineSettings(agent1);
				await agent1.offlinePage.setLocalMailboxEnabled(true);
				await returnToChat(agent1, 'Bob');
			});

			after(async () => {
				await openOfflineSettings(agent1);
				await agent1.offlinePage.setLocalMailboxEnabled(false);
				await returnToChat(agent1, 'Bob');
			});

			it('navbar chip switches from "disconnected" to "local" once the local mailbox is discovered and polled', async () => {
				await agent1.waitUntil(
					async () => (await agent1.directChatPage.connectionStatus()) === 'local',
					{ timeout: 60_000 },
				);
			});

			it('a new message advances to the "local" mailbox icon', async () => {
				await agent1.directChatPage.sendMessage('local hello');
				await agent1.waitUntil(
					async () => (await agent1.directChatPage.lastMessageStatus()) === 'local',
					{ timeout: 30_000 },
				);
			});
		});
	});

	describe('cloud mailbox back online', () => {
		it('navbar chip hides again and pending message advances to "cloud"', async () => {
			await agent1.waitUntil(
				async () => (await agent1.directChatPage.connectionStatus()) === 'connected',
				{ timeout: 30_000 },
			);
			await agent1.waitUntil(
				async () => (await agent1.directChatPage.lastMessageStatus()) === 'cloud',
				{ timeout: 30_000 },
			);
			await agent2.directChatPage.waitForMessage('offline hello');
		});
	});
});
