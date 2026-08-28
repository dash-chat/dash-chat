/**
 * Offline UX E2E.
 *
 * Drives the per-message status indicator and the chat navbar connection chip
 * through every visible state by suspending the global cloud mailbox and
 * toggling the per-agent local mailbox server underneath the running agents.
 *
 * Visible states under test:
 *   - Message status: "sending" → "mailbox" → "delivered"
 *   - Navbar chip:    hidden (connected) → disconnected → local → hidden (connected)
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

async function openOfflineSettings(agent: Agent): Promise<void> {
	await agent.directChatPage.back.click();
	await agent.homePage.ready();
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
		// The whole suite toggles the mailbox server's lifecycle, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		// agent1 enables the local mailbox, which is desktop-only.
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'any' },
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
		it('sends a message, peer receives it, sender shows delivered, and the navbar chip stays hidden', async () => {
			await agent1.directChatPage.composer.sendMessage('online hello');
			await agent2.directChatPage.messages.waitForMessage('online hello');

			// The peer received it, so the peer's ack must eventually flip the
			// indicator to the double check.
			await agent1.directChatPage.messages.waitForMessageStatus(
				'online hello',
				['delivered'],
			);
			expect(
				await agent1.directChatPage.connectionStatusIndicator.status(),
			).toBe('connected');
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
			await agent1.directChatPage.composer.sendMessage('offline hello');
			await agent1.waitUntil(
				async () =>
					(await agent1.directChatPage.lastMessageStatus()) === 'sending',
				{ timeout: 5_000 },
			);
			await agent1.pause(5_000);
			expect(await agent1.directChatPage.lastMessageStatus()).toBe('sending');
		});

		// connect_timeout=5s + timeout=10s × degraded_threshold(5) gives a
		// worst case around ~60s before the chip flips. Pad on top so a slow
		// runner doesn't false-fail.
		it('navbar chip flips to "disconnected", clicking it opens the explainer dialog, and the close button dismisses it', async () => {
			const indicator = agent1.directChatPage.connectionStatusIndicator;
			await agent1.waitUntil(
				async () => (await indicator.status()) === 'disconnected',
				{ timeout: 90_000, interval: 1_000 },
			);

			await indicator.chip.click();
			await agent1.waitUntil(() => indicator.isDialogOpen());
			await expect(indicator.dialogTitle).toHaveText(
				await agent1.tr('connectionStatusDisconnectedTitle'),
			);
			await expect(indicator.dialogDescription).toHaveText(
				await agent1.tr('connectionStatusDisconnectedDescription'),
			);

			await indicator.dialogCloseButton.click();
			await agent1.waitUntil(async () => !(await indicator.isDialogOpen()));
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

			it('navbar chip switches from "disconnected" to "local" once the local mailbox is discovered, and the dialog reports the local-mailbox state', async () => {
				const indicator = agent1.directChatPage.connectionStatusIndicator;
				await agent1.waitUntil(
					async () => (await indicator.status()) === 'local',
					{ timeout: 60_000 },
				);

				await indicator.chip.click();
				await agent1.waitUntil(() => indicator.isDialogOpen());
				await expect(indicator.dialogTitle).toHaveText(
					await agent1.tr('connectionStatusLocalTitle'),
				);
				await expect(indicator.dialogDescription).toHaveText(
					await agent1.tr('connectionStatusLocalDescription', { count: 1 }),
				);

				await indicator.dialogCloseButton.click();
				await agent1.waitUntil(async () => !(await indicator.isDialogOpen()));
			});

			it('a new message advances to the "mailbox" icon once the local mailbox holds it', async () => {
				await agent1.directChatPage.composer.sendMessage('local hello');
				await agent1.directChatPage.messages.waitForMessageStatus(
					'local hello',
					// Only agent1 reaches the local mailbox, so the message can't
					// become delivered — it settles on 'mailbox'.
					['mailbox'],
				);
			});
		});
	});

	describe('cloud mailbox back online', () => {
		it('navbar chip hides again and the pending message advances to delivered once the peer receives it', async () => {
			await agent1.waitUntil(
				async () =>
					(await agent1.directChatPage.connectionStatusIndicator.status()) ===
					'connected',
				{ timeout: 30_000 },
			);
			await agent2.directChatPage.messages.waitForMessage('offline hello');
			await agent1.directChatPage.messages.waitForMessageStatus(
				'offline hello',
				['delivered'],
			);
		});
	});

	// A burst of messages whose delivery statuses differ must not collapse into
	// one visual group with a single indicator on the last message: the group
	// splits at every status boundary so each status stays visible, and merges
	// back once the statuses converge again.
	describe('messages with different delivery statuses', () => {
		before(async function () {
			this.timeout(120_000);
			await openOfflineSettings(agent1);
			await agent1.offlinePage.setLocalMailboxEnabled(true);
			await returnToChat(agent1, 'Bob');
		});

		after(async function () {
			this.timeout(120_000);
			if (mailboxSuspended) {
				resumeMailbox();
				mailboxSuspended = false;
			}
			await openOfflineSettings(agent1);
			await agent1.offlinePage.setLocalMailboxEnabled(false);
			await returnToChat(agent1, 'Bob');
		});

		it('split their group so every status stays visible', async function () {
			this.timeout(180_000);
			await agent1.directChatPage.composer.sendMessage('split delivered');
			await agent1.directChatPage.messages.waitForMessageStatus(
				'split delivered',
				['delivered'],
			);

			suspendMailbox();
			mailboxSuspended = true;

			await agent1.directChatPage.composer.sendMessage('split mailbox');
			// Generous timeout: the just-enabled local mailbox may still be
			// waiting on mDNS discovery before it can hold the message.
			await agent1.directChatPage.messages.waitForMessageStatus(
				'split mailbox',
				['mailbox'],
				60_000,
			);

			await openOfflineSettings(agent1);
			await agent1.offlinePage.setLocalMailboxEnabled(false);
			await returnToChat(agent1, 'Bob');
			await agent1.directChatPage.composer.sendMessage('split sending');
			await agent1.directChatPage.messages.waitForMessageStatus(
				'split sending',
				['sending'],
			);

			// The sends above land within the one-minute grouping window, so
			// without status-based splitting only the last message would carry
			// an indicator. Every message must show its own status at once.
			expect(
				await agent1.directChatPage.messages.messageStatusFor('split mailbox'),
			).toBe('mailbox');
			expect(
				await agent1.directChatPage.messages.messageStatusFor(
					'split delivered',
				),
			).toBe('delivered');
		});

		it('merge back into one group once the statuses converge', async function () {
			this.timeout(120_000);
			resumeMailbox();
			mailboxSuspended = false;

			await agent1.directChatPage.messages.waitForMessageStatus(
				'split sending',
				['delivered'],
			);
			// "split mailbox" and "split sending" were sent seconds apart, so
			// they share the grouping window: once both are delivered the group
			// merges again and only its last message keeps an indicator.
			await agent1.waitUntil(
				async () =>
					(await agent1.directChatPage.messages.messageStatusFor(
						'split mailbox',
					)) === null,
				{
					timeoutMsg:
						'converged message kept its indicator instead of merging back into the group',
				},
			);
		});
	});

	// Regression: a message delivered to the cloud must still read as delivered
	// after the app restarts while the cloud mailbox is unreachable. The cloud
	// mailbox id is resolved from the live server, so on a cold start against an
	// unreachable server it can't be re-resolved — but the delivered status is
	// recorded in persisted sync state and must survive regardless.
	describe('app restarts while the cloud mailbox is unreachable', () => {
		let mailboxKilled = false;

		after(async function () {
			if (!mailboxKilled) return;
			this.timeout(60_000);
			await restartMailbox();
			mailboxKilled = false;
		});

		it('a message at the mailbox still shows its status after restarting with the mailbox down', async function () {
			this.timeout(120_000);
			// Deliver a fresh message to the cloud right now (mailbox is online).
			// It may already advance to 'delivered' if the peer's ack races in.
			await agent1.directChatPage.composer.sendMessage('restart hello');
			await agent1.directChatPage.messages.waitForMessageStatus(
				'restart hello',
				['mailbox', 'delivered'],
			);

			// Kill the cloud mailbox so it is unreachable, then cold-start the app.
			// On boot the app re-resolves the cloud mailbox id from the live server;
			// with the server down it never can, but the delivered status lives in
			// persisted sync state and must survive.
			killMailbox();
			mailboxKilled = true;
			await agent1.restart();

			await agent1.homePage.ready();
			await agent1.homePage.openChat('Bob');
			await agent1.directChatPage.ready();

			let status =
				await agent1.directChatPage.messageStatusFor('restart hello');
			await agent1.waitUntil(
				async () => {
					status =
						await agent1.directChatPage.messageStatusFor('restart hello');
					return status !== null;
				},
				{
					timeout: 15_000,
					interval: 500,
					timeoutMsg: 'message status indicator never rendered after restart',
				},
			);
			expect(['mailbox', 'delivered']).toContain(status);
		});
	});
});
