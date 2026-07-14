/**
 * Regression: a client's connection to a local mailbox server must survive the
 * mDNS announcement TTL (120s for SRV/address records) — the records must be
 * refreshed in the client's cache instead of expiring and unregistering the
 * mailbox.
 *
 * agent1 hosts the in-process local mailbox server; agent2 is the client. The
 * cloud mailbox is suspended so agent2's navbar chip becomes visible and shows
 * the "local" icon once agent1's server is discovered. The chip must still
 * read "local" after 3 minutes — comfortably past the TTL.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Local mailbox connection survives the mDNS announcement TTL', () => {
	let agent1: Agent;
	let agent2: Agent;
	let mailboxSuspended = false;

	before(async function () {
		this.timeout(120_000);
		// The suite suspends the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		// exchangeContacts leaves agent2 inside its direct chat with Alice —
		// that's where ConnectionStatusIndicator is mounted.
		await exchangeContacts(agent1, agent2);

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.settingsLink.click();
		await agent1.settingsPage.ready();
		await agent1.settingsPage.offlineLink.click();
		await agent1.offlinePage.ready();
		await agent1.offlinePage.setLocalMailboxEnabled(true);

		suspendMailbox();
		mailboxSuspended = true;
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

	// connect_timeout=5s + timeout=10s per hanging cloud request, and the UI
	// flips after 2 consecutive errors; pad for mDNS discovery of the local
	// mailbox on top.
	it('client shows the local mailbox icon once connected to the peer server', async function () {
		this.timeout(120_000);
		const indicator = agent2.directChatPage.connectionStatusIndicator;
		await agent2.waitUntil(
			async () => (await indicator.status()) === 'local',
			{ timeout: 90_000, interval: 1_000 },
		);
	});

	it('keeps showing the local mailbox icon for 3 minutes, outliving the mDNS TTL', async function () {
		this.timeout(240_000);
		const indicator = agent2.directChatPage.connectionStatusIndicator;
		const deadline = Date.now() + 180_000;
		while (Date.now() < deadline) {
			expect(await indicator.status()).toBe('local');
			await agent2.pause(10_000);
		}
		expect(await indicator.status()).toBe('local');
	});
});
