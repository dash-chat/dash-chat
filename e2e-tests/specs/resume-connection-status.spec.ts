/**
 * Regression: resuming from the background must not greet the user with the
 * "disconnected" chip for a connection failure that happened while the app was
 * away.
 *
 * Android denies network access to backgrounded apps, so the mailbox polls that
 * run while we are gone fail and leave the cloud mailbox backed off with a high
 * consecutive-error count. Foregrounding has to clear that stale verdict and
 * re-measure, rather than reporting a connection the user no longer lacks.
 *
 * The suspended cloud mailbox stands in for the background network denial: it
 * is what makes the polls fail. It stays suspended across the whole
 * background/foreground cycle, so nothing except the resume itself can clear
 * the chip — and so the chip has to come back once the fresh polls fail too.
 */
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so `this.timeout()` inside a test body comes too late — the timeout
// must be set suite-wide, before the tests start.
describe('Connection status after resuming from the background', function () {
	this.timeout(300_000);

	let agent: Agent;
	let mailboxSuspended = false;

	before(async function () {
		// The suite suspends the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		// The resume hook that wakes the cloud mailbox is Android-only, and
		// `backgroundApp` is a no-op on desktop.
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.createProfilePage.createProfile('Alice', 'Test');
		await agent.enablePreviewFeatures();

		// A members-less group chat is the cheapest way to a page where
		// ConnectionStatusIndicator is mounted.
		await agent.homePage.newMessageButton.click();
		await agent.newMessagePage.ready();
		await agent.newMessagePage.newGroup.click();
		await agent.newGroupPage.addMembersStep.ready();
		await agent.newGroupPage.addMembersStep.nextButton.click();
		await agent.newGroupPage.groupInfoStep.ready();
		await agent.newGroupPage.groupInfoStep.setName('Solo Group');
		await agent.newGroupPage.groupInfoStep.createButton.click();
		await agent.groupChatPage.ready();
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
	// flips after 3 consecutive errors; pad so a slow runner doesn't false-fail.
	it('shows the disconnected chip once the cloud mailbox stops answering', async () => {
		suspendMailbox();
		mailboxSuspended = true;

		const indicator = agent.groupChatPage.connectionStatusIndicator;
		await agent.waitUntil(
			async () => (await indicator.status()) === 'disconnected',
			{ timeout: 90_000, interval: 1_000 },
		);
	});

	it('hides the chip on resume rather than replaying the failure from the background', async () => {
		await agent.backgroundApp();
		await agent.startApp();
		await agent.groupChatPage.ready();

		// `startApp` re-attaches the page objects, so this must be read after it.
		const indicator = agent.groupChatPage.connectionStatusIndicator;
		await agent.waitUntil(
			async () => (await indicator.status()) !== 'disconnected',
			{
				timeoutMsg:
					'the chip still showed the pre-background failure after resuming',
			},
		);
	});

	it('brings the chip back once the polls made after the resume fail too', async () => {
		const indicator = agent.groupChatPage.connectionStatusIndicator;
		await agent.waitUntil(
			async () => (await indicator.status()) === 'disconnected',
			{ timeout: 90_000, interval: 1_000 },
		);
	});
});
