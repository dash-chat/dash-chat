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
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgent } from '../setup/setup-agents';

// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so `this.timeout()` inside a test body comes too late — the timeout
// must be set suite-wide, before the tests start.
describe('Local mailbox connection survives the mDNS announcement TTL', function () {
	this.timeout(240_000);

	let agent1: Agent;
	let agent2: Agent;
	let mailboxSuspended = false;

	before(async function () {
		// The suite suspends the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		// agent1 hosts the local mailbox server.
		await agent1.homePage.settingsLink.click();
		await agent1.settingsPage.ready();
		await agent1.settingsPage.offlineLink.click();
		await agent1.offlinePage.ready();
		await agent1.offlinePage.setLocalMailboxEnabled(true);

		// agent2 opens a members-less group chat — the cheapest way to a page
		// where ConnectionStatusIndicator is mounted.
		await agent2.homePage.newMessageButton.click();
		await agent2.newMessagePage.ready();
		await agent2.newMessagePage.newGroup.click();
		await agent2.newGroupPage.addMembersStep.ready();
		await agent2.newGroupPage.addMembersStep.nextButton.click();
		await agent2.newGroupPage.groupInfoStep.ready();
		await agent2.newGroupPage.groupInfoStep.setName('Solo Group');
		await agent2.newGroupPage.groupInfoStep.createButton.click();
		await agent2.groupChatPage.ready();

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
	it('client shows the local mailbox icon once connected to the peer server', async () => {
		const indicator = agent2.groupChatPage.connectionStatusIndicator;
		await agent2.waitUntil(
			async () => (await indicator.status()) === 'local',
			{ timeout: 90_000, interval: 1_000 },
		);
	});

	it('keeps showing the local mailbox icon for 3 minutes, outliving the mDNS TTL', async () => {
		const indicator = agent2.groupChatPage.connectionStatusIndicator;
		const deadline = Date.now() + 180_000;
		while (Date.now() < deadline) {
			expect(await indicator.status()).toBe('local');
			await agent2.pause(10_000);
		}
		expect(await indicator.status()).toBe('local');
	});
});
