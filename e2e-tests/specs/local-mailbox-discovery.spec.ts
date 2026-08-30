/**
 * A hub only announces itself when it registers, and mdns-sd doubles its browse
 * re-query interval up to an hour — so a missed announcement leaves the hub
 * undiscovered until the app is restarted. The first case starts the hub with
 * the app foregrounded (control: the announcement is heard); the second starts
 * it while the app is backgrounded, which is the field scenario.
 *
 * A phone that dropped to cellular, or lost Wi-Fi, fails this identically to the
 * bug — check it still holds a LAN address before believing a red.
 *
 * Skips itself unless E2E_STRESS=1:
 *   PLATFORMS=android,desktop E2E_STRESS=1 just e2e run local-mailbox-discovery
 *
 * Tunables: E2E_STRESS_UPTIME_SECONDS, E2E_STRESS_DISCOVERY_SECONDS,
 * E2E_STRESS_DEAFEN_SECONDS, E2E_STRESS_UNHEARD_SECONDS.
 */
import { type ConnectionStatusIndicator } from '../helpers/components/connection-status-indicator';
import { envInt } from '../helpers/utils';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const UPTIME_MS = envInt('E2E_STRESS_UPTIME_SECONDS', 20) * 1_000;
const DISCOVERY_MS = envInt('E2E_STRESS_DISCOVERY_SECONDS', 20) * 1_000;
const SOAK_POLL_MS = 30_000;
/** Time for the OS to stop delivering to the backgrounded app, before the hub's
 *  announcement is sent. */
const DEAFEN_MS = envInt('E2E_STRESS_DEAFEN_SECONDS', 60) * 1_000;
/** How long the announcement is left unheard before the app comes back. */
const UNHEARD_MS = envInt('E2E_STRESS_UNHEARD_SECONDS', 5) * 1_000;

/**
 * Hold until `deadline` while keeping the chip under observation. The polling is
 * not decoration: an idle session is torn down after its driver's command
 * timeout, so both agents have to be touched or one dies before the hub starts.
 */
async function soakWithoutHub(
	phone: Agent,
	host: Agent,
	indicator: ConnectionStatusIndicator,
	deadline: number,
): Promise<void> {
	while (Date.now() < deadline) {
		await phone.pause(Math.min(SOAK_POLL_MS, deadline - Date.now()));
		await host.offlinePage.localMailboxToggle.isExisting();
		const status = await indicator.status();
		if (status === 'local') {
			throw new Error(
				'a local hub was discovered during the soak; the LAN is not clean, so the run proves nothing',
			);
		}
	}
}

// wdio arms its per-test abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside the test body.
describe('Local mailbox discovery', function () {
	this.timeout(UPTIME_MS + DISCOVERY_MS + 300_000);

	let phone: Agent;
	let host: Agent;
	let mailboxSuspended = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The suite suspends the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		// The hub is a desktop-only feature; the discovering side can be any
		// phone, since the mDNS stack is shared and iOS blocks multicast without
		// the networking.multicast entitlement.
		[phone, host] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'desktop' },
		]);
		// An emulator is NAT'd off the runner's LAN, so it could never see the
		// hub — that would be a red for infrastructure reasons, not the bug.
		if (phone.platform === 'android-emulator') this.skip();
		await phone.createProfilePage.createProfile('Alice', 'Test');
		await host.createProfilePage.createProfile('Bob', 'Test');

		// Park the host on the page that starts the hub, so that step is a
		// single click once the soak is over.
		await host.homePage.settingsLink.click();
		await host.settingsPage.ready();
		await host.settingsPage.offlineLink.click();
		await host.offlinePage.ready();

		// A members-less group chat is the cheapest way to a page where
		// ConnectionStatusIndicator is mounted.
		await phone.homePage.newMessageButton.click();
		await phone.newMessagePage.ready();
		await phone.newMessagePage.newGroup.click();
		await phone.newGroupPage.addMembersStep.ready();
		await phone.newGroupPage.addMembersStep.nextButton.click();
		await phone.newGroupPage.groupInfoStep.ready();
		await phone.newGroupPage.groupInfoStep.setName('Solo Group');
		await phone.newGroupPage.groupInfoStep.createButton.click();
		await phone.groupChatPage.ready();

		suspendMailbox();
		mailboxSuspended = true;
	});

	after(() => {
		if (!mailboxSuspended) return;
		try {
			resumeMailbox();
		} catch {
			/* mailbox process already gone */
		}
		mailboxSuspended = false;
	});

	it('discovers a hub that starts while the app is in the foreground', async () => {
		// Re-anchor the browse schedule: everything below is timed from here, so
		// the hub lands where it is meant to inside the doubling interval.
		await phone.restart();
		const browseStartedAt = Date.now();
		await phone.homePage.ready();
		await phone.homePage.chatRow.click();
		await phone.groupChatPage.ready();

		const indicator = phone.groupChatPage.connectionStatusIndicator;

		await phone.waitUntil(
			async () => (await indicator.status()) === 'disconnected',
			{
				timeout: 90_000,
				interval: 1_000,
				timeoutMsg: 'client never reported itself disconnected',
			},
		);

		await soakWithoutHub(phone, host, indicator, browseStartedAt + UPTIME_MS);

		await host.offlinePage.setLocalMailboxEnabled(true);
		const startedAt = Date.now();

		await phone.waitUntil(async () => (await indicator.status()) === 'local', {
			timeout: DISCOVERY_MS,
			interval: 1_000,
			timeoutMsg:
				`hub started at ${UPTIME_MS / 1_000}s of app uptime was still undiscovered ` +
				`${DISCOVERY_MS / 1_000}s later; restarting the app would find it immediately`,
		});
		console.log(
			`hub discovered ${Math.round((Date.now() - startedAt) / 1000)}s after it started`,
		);
	});

	it('discovers a hub that started while the app was backgrounded', async () => {
		const indicator = phone.groupChatPage.connectionStatusIndicator;

		await host.offlinePage.setLocalMailboxEnabled(false);
		await phone.waitUntil(
			async () => (await indicator.status()) === 'disconnected',
			{
				timeout: 150_000,
				interval: 1_000,
				timeoutMsg: 'client still saw a hub after the host stopped it',
			},
		);

		await phone.backgroundApp();
		await host.pause(DEAFEN_MS);

		// The announcement goes out while the app is away, so it is lost — and a
		// hub never announces again.
		await host.offlinePage.setLocalMailboxEnabled(true);
		await host.pause(UNHEARD_MS);

		await phone.startApp();
		await phone.groupChatPage.ready();
		const resumedAt = Date.now();

		await phone.waitUntil(async () => (await indicator.status()) === 'local', {
			timeout: DISCOVERY_MS,
			interval: 1_000,
			timeoutMsg:
				'hub that started while the app was backgrounded was still undiscovered ' +
				`${DISCOVERY_MS / 1_000}s after coming back; only an app restart would find it`,
		});
		console.log(
			`hub discovered ${Math.round((Date.now() - resumedAt) / 1000)}s after resuming`,
		);
	});
});
