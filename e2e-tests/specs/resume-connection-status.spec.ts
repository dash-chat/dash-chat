/**
 * Regression: returning to the app after a long time away must not show the
 * "disconnected" chip for polls that failed while it was backgrounded.
 *
 * Nothing here simulates the failure. The mailbox stays up and reachable the
 * whole time; the spec only backgrounds the app, waits, and comes back, which
 * is the entire scenario. On a device whose OS lets a backgrounded app keep
 * polling, nothing fails and this passes on any build — that is the intended
 * outcome, not a gap. On a device that denies a backgrounded app the network,
 * the polls fail for real, and the chip must still not be on screen when the
 * user returns.
 *
 * The chip's own reporting — flipping to "disconnected" when the mailbox is
 * genuinely unreachable, and back — is covered by `offline-transition-ux`.
 */
import {
	formatStatusTrace,
	samplesOnResume,
} from '../helpers/components/connection-status-indicator';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** How long the app stays backgrounded. Android cuts a backgrounded app's
 *  network within seconds — measured at 5s to the first failed poll and 10s to
 *  the three that earn the chip — so this only needs to clear that with margin.
 *  Waiting longer changes nothing: the same device throttles identically at 30s
 *  and at two minutes. */
const BACKGROUND_MS = 20_000;

/** How long the connection must hold up in the foreground before the app is
 *  sent away. The gate can only see instability once it is bad enough to render
 *  the chip, which takes three consecutive failures — about 15s on the flakiest
 *  link measured — so it cannot go much below this without missing exactly the
 *  case it exists to catch. */
const FOREGROUND_HEALTHY_MS = 20_000;

// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so `this.timeout()` inside a test body comes too late — the timeout
// must be set suite-wide, before the tests start.
describe('Connection status after resuming from the background', function () {
	this.timeout(420_000);

	let agent: Agent;

	before(async function () {
		// The lifecycle hooks are Android-only, and `backgroundApp` is a no-op on
		// desktop.
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.createProfilePage.createProfile('Alice', 'Test');
		await agent.enablePreviewFeatures();

		// Background mode is deliberately left at its default (off), which is what
		// users have: it is the foreground service that keeps the process alive,
		// and enabling it is precisely what stops Android reclaiming the app —
		// the thing this spec is about.

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

	it('does not show the disconnected chip on returning from a long background', async () => {
		const indicator = agent.groupChatPage.connectionStatusIndicator;

		// Let startup settle: the cloud mailbox is registered after a `/health`
		// round trip, so the chip can legitimately be up for a moment first.
		// `!== 'disconnected'` rather than `=== 'connected'`: a local mailbox on
		// the network shows the "local" chip, which is not the poop.
		await agent.waitUntil(
			async () => (await indicator.status()) !== 'disconnected',
			{ timeout: 90_000, interval: 1_000 },
		);

		// Then hold still and prove it stays that way. A link that flaps while the
		// app is in use tells us nothing about the time away: a chip seen after
		// the resume could have been earned there and then rather than inherited
		// from the background, and the run would convict the app of a bug it did
		// not commit. Recorded rather than sampled so a single-frame flash counts.
		const foregroundToken = await indicator.startRecordingStatus();
		await agent.pause(FOREGROUND_HEALTHY_MS);
		const foreground = await indicator.recordedStatuses(foregroundToken);
		if (foreground.some(sample => sample.status === 'disconnected')) {
			throw new Error(
				`the connection is not stable while the app is in the foreground, so this run ` +
					`cannot tell an inherited verdict from a freshly earned one ` +
					`(rendered: ${formatStatusTrace(foreground)})`,
			);
		}

		const token = await indicator.startRecordingStatus();

		await agent.backgroundApp();
		await agent.pause(BACKGROUND_MS);
		await agent.startApp();
		await agent.groupChatPage.ready();

		// `recordedStatuses` throws if the recording did not survive — the webview
		// is then a new one, meaning Android reclaimed the app, and what was on
		// screen in between was never observed. That is inconclusive, not a pass.
		const onScreen = samplesOnResume(
			await agent.groupChatPage.connectionStatusIndicator.recordedStatuses(
				token,
			),
		);
		if (onScreen === null) {
			throw new Error(
				'the page never reported becoming visible, so what the user saw on resume is unknown',
			);
		}
		if (onScreen.some(sample => sample.status === 'disconnected')) {
			throw new Error(
				`the chip showed a verdict inherited from the time away ` +
					`(on screen as it came back: ${formatStatusTrace(onScreen)})`,
			);
		}
	});
});
