/**
 * Regression: resuming from the background must not greet the user with the
 * "disconnected" chip for a connection failure that happened while the app was
 * away.
 *
 * Android denies network access to backgrounded apps, so the mailbox polls that
 * run while we are gone fail through no fault of the connection the user comes
 * back to. Those failures must not be recorded, and foregrounding has to
 * re-measure rather than repaint a verdict formed while the app had no way of
 * reaching anything.
 *
 * Suspending the cloud mailbox stands in for that network denial: it is what
 * makes the polls fail, and it is confined to the background window so the app
 * is demonstrably connected on both sides of it.
 */
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** Long enough for several polls to time out and clear the UI's 3-error
 *  threshold: a hanging poll gives up after ~10s, and the next follows ~2.5s
 *  later. Without the fix this window is what puts the chip on screen, so it
 *  needs margin over the bare 3 failures the threshold asks for. */
const BACKGROUND_FAILURE_MS = 60_000;

// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so `this.timeout()` inside a test body comes too late — the timeout
// must be set suite-wide, before the tests start.
describe('Connection status after resuming from the background', function () {
	this.timeout(300_000);

	let agent: Agent;
	let mailboxSuspended = false;

	const suspend = () => {
		suspendMailbox();
		mailboxSuspended = true;
	};
	const resume = () => {
		if (!mailboxSuspended) return;
		resumeMailbox();
		mailboxSuspended = false;
	};

	before(async function () {
		// The suite suspends the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		// The resume hook that clears the background failures is Android-only,
		// and `backgroundApp` is a no-op on desktop.
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.createProfilePage.createProfile('Alice', 'Test');
		await agent.enablePreviewFeatures();

		// Without background mode Android freezes the process on background, so
		// no poll runs to fail and there is nothing for the fix to suppress —
		// the test would pass on any build. The foreground service keeps the
		// node polling, which is both what makes this a regression test and the
		// configuration the bug was reported under.
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineLink.click();
		await agent.offlinePage.ready();
		await agent.offlinePage.setBackgroundModeEnabled(true);
		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.back.click();
		await agent.homePage.ready();

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
		try {
			resume();
		} catch {
			/* the run is already over; a stuck-suspended server is cleaned up
			   with the rest of the fixture */
		}
	});

	// Guards the fix below on two fronts: suppressing background failures must
	// not cost us the ability to report a foreground one, and the recorder the
	// next test relies on must be able to see a disconnection that definitely
	// happened — otherwise its silence there would prove nothing.
	it('shows the disconnected chip while the cloud mailbox is unreachable in the foreground', async () => {
		const indicator = agent.groupChatPage.connectionStatusIndicator;
		const token = await indicator.startRecordingStatus();
		suspend();
		await agent.waitUntil(
			async () => (await indicator.status()) === 'disconnected',
			{ timeout: 90_000, interval: 1_000 },
		);

		const statuses = await indicator.recordedStatuses(token);
		if (!statuses.includes('disconnected')) {
			throw new Error(
				`the status recorder missed a disconnection that was on screen (statuses rendered: ${statuses.join(' -> ')})`,
			);
		}

		resume();
		await agent.waitUntil(
			async () => (await indicator.status()) !== 'disconnected',
			{ timeout: 90_000, interval: 1_000 },
		);
	});

	it('does not report the polls that failed while backgrounded as a disconnection', async () => {
		// Recorded rather than read after the fact: the backend re-measures within
		// ~40ms of foregrounding, so a chip painted from the background failures is
		// gone long before a post-resume read happens — but the user still saw it.
		const token =
			await agent.groupChatPage.connectionStatusIndicator.startRecordingStatus();

		await agent.backgroundApp();
		suspend();
		await agent.pause(BACKGROUND_FAILURE_MS);
		// The connection is back before the user is, exactly as it is when
		// Android restores network access to a foregrounded app.
		resume();

		await agent.startApp();
		await agent.groupChatPage.ready();

		// `startApp` re-attaches the page objects, so this must be read after it.
		const indicator = agent.groupChatPage.connectionStatusIndicator;
		const statuses = await indicator.recordedStatuses(token);
		if (statuses.includes('disconnected')) {
			throw new Error(
				`the chip showed the failures that happened while the app was backgrounded (statuses rendered: ${statuses.join(' -> ')})`,
			);
		}
	});
});
