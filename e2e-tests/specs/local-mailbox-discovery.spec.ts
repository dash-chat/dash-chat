/**
 * A hub only announces itself when it registers, and mdns-sd doubles its browse
 * re-query interval up to an hour — so a missed announcement leaves the hub
 * undiscovered until the app is restarted. The first case starts the hub with
 * the app foregrounded (control: the announcement is heard); the second starts
 * it while the app is backgrounded, which is the field scenario.
 *
 * The third and fourth cases target multicast *reception* rather than a missed
 * announcement. Android's Wi-Fi driver filters multicast not addressed to the
 * device unless the app holds a `WifiManager.MulticastLock`, so mDNS replies to
 * 224.0.0.251 are dropped opportunistically. One cycle hides that behind
 * mdns-sd's re-query; repeating it surfaces the loss as a *rate* — the lock
 * makes reception good, not perfect, so the soak scores rounds and tolerates
 * `MAX_MISSED` rather than demanding a clean sweep. The Wi-Fi bounce reproduces
 * the field report directly (measured once at 6m40s between joining the hub's
 * network and the hub appearing).
 *
 * A phone that dropped to cellular, or lost Wi-Fi, fails this identically to the
 * bug — check it still holds a LAN address before believing a red.
 *
 * Skips itself unless E2E_STRESS=1:
 *   PLATFORMS=android,desktop E2E_STRESS=1 just e2e run local-mailbox-discovery
 *
 * Tunables: E2E_STRESS_UPTIME_SECONDS, E2E_STRESS_DISCOVERY_SECONDS,
 * E2E_STRESS_DEAFEN_SECONDS, E2E_STRESS_UNHEARD_SECONDS,
 * E2E_STRESS_CYCLES, E2E_STRESS_HEARD_SECONDS, E2E_STRESS_MAX_MISSED,
 * E2E_STRESS_WIFI_DOWN_SECONDS.
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
/** Hub stop/start rounds in the soak. One round only ever samples a single
 *  announcement, so it cannot tell a lossy link from a broken one; several
 *  rounds turn that into a rate worth judging against [`MAX_MISSED`]. */
const CYCLES = envInt('E2E_STRESS_CYCLES', 5);
/** What "the announcement was heard" is allowed to cost: the unsolicited
 *  multicast reaches a listening app at once, leaving only the registration
 *  path's TCP probe (2s budget) and health fetch. Anything beyond this means
 *  the announcement was missed and mdns-sd's re-query (1s, doubling) found it
 *  instead. */
const HEARD_MS = envInt('E2E_STRESS_HEARD_SECONDS', 8) * 1_000;
/** Multicast is unacknowledged, so a dropped announcement on a busy 2.4GHz
 *  network is physics rather than a defect — demanding a clean sweep would flake
 *  no matter how good the fix. The bug this guards against missed *every* round,
 *  so tolerating one still fails loudly on a real regression. */
const MAX_MISSED = envInt('E2E_STRESS_MAX_MISSED', 1);
/** Long enough that the supplicant fully tears the association down rather than
 *  treating the bounce as a blip. */
const WIFI_DOWN_MS = envInt('E2E_STRESS_WIFI_DOWN_SECONDS', 8) * 1_000;
/** A hub whose goodbye packet is lost lingers until its records age out, so
 *  teardown gets far more room than discovery. */
const TEARDOWN_MS = 150_000;

/** Wait for the connection chip to settle on `status`. */
async function waitForStatus(
	phone: Agent,
	indicator: ConnectionStatusIndicator,
	status: 'local' | 'disconnected',
	timeout: number,
	timeoutMsg: string,
): Promise<void> {
	await phone.waitUntil(async () => (await indicator.status()) === status, {
		timeout,
		interval: 1_000,
		timeoutMsg,
	});
}

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
	this.timeout(
		Math.max(UPTIME_MS + DISCOVERY_MS, CYCLES * (TEARDOWN_MS + HEARD_MS)) +
			300_000,
	);

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
		// A phone that fell back to cellular fails every case here in exactly the
		// shape of the bug, so refuse to run rather than report four red herrings.
		if (phone.platform === 'android' && (await phone.wifiAddress()) === '') {
			throw new Error(
				'phone has no wifi address (fell back to cellular?), so it cannot reach a hub ' +
					"on the host's LAN — reconnect it to the host's network before reading anything " +
					'into this suite',
			);
		}
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

		await waitForStatus(
			phone,
			indicator,
			'disconnected',
			90_000,
			'client never reported itself disconnected',
		);

		await soakWithoutHub(phone, host, indicator, browseStartedAt + UPTIME_MS);

		await host.offlinePage.setLocalMailboxEnabled(true);
		const startedAt = Date.now();

		await waitForStatus(
			phone,
			indicator,
			'local',
			DISCOVERY_MS,
			`hub started at ${UPTIME_MS / 1_000}s of app uptime was still undiscovered ` +
				`${DISCOVERY_MS / 1_000}s later; restarting the app would find it immediately`,
		);
		console.log(
			`hub discovered ${Math.round((Date.now() - startedAt) / 1000)}s after it started`,
		);
	});

	it('discovers a hub that started while the app was backgrounded', async () => {
		const indicator = phone.groupChatPage.connectionStatusIndicator;

		await host.offlinePage.setLocalMailboxEnabled(false);
		await waitForStatus(
			phone,
			indicator,
			'disconnected',
			TEARDOWN_MS,
			'client still saw a hub after the host stopped it',
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

		await waitForStatus(
			phone,
			indicator,
			'local',
			DISCOVERY_MS,
			'hub that started while the app was backgrounded was still undiscovered ' +
				`${DISCOVERY_MS / 1_000}s after coming back; only an app restart would find it`,
		);
		console.log(
			`hub discovered ${Math.round((Date.now() - resumedAt) / 1000)}s after resuming`,
		);
	});

	it('hears hub announcements reliably across repeated restarts', async () => {
		const indicator = phone.groupChatPage.connectionStatusIndicator;
		// `null` for a round whose announcement never arrived.
		const latencies: (number | null)[] = [];

		for (let cycle = 1; cycle <= CYCLES; cycle++) {
			await host.offlinePage.setLocalMailboxEnabled(false);
			// A round that missed leaves the chip already disconnected, so this
			// settles immediately rather than waiting out the teardown budget.
			await waitForStatus(
				phone,
				indicator,
				'disconnected',
				TEARDOWN_MS,
				`hub still showed as connected ${TEARDOWN_MS / 1_000}s into cycle ${cycle}'s teardown`,
			);

			await host.offlinePage.setLocalMailboxEnabled(true);
			const startedAt = Date.now();
			// Rounds are scored, not asserted: one miss is tolerated below, so a
			// failure here has to be recorded and the soak carried on.
			const heardIn = await waitForStatus(
				phone,
				indicator,
				'local',
				HEARD_MS,
				'',
			).then(
				() => Date.now() - startedAt,
				() => null,
			);
			latencies.push(heardIn);
			// Logged per cycle, not just in the summary: on a flaky device the
			// session can die mid-soak, and the rounds that did complete are the
			// evidence that survives.
			console.log(
				heardIn === null
					? `cycle ${cycle}/${CYCLES}: announcement NOT heard within ${HEARD_MS / 1_000}s`
					: `cycle ${cycle}/${CYCLES}: announcement heard in ${heardIn}ms`,
			);
		}

		const missed = latencies.filter(heardIn => heardIn === null).length;
		const distribution = latencies
			.map(heardIn => (heardIn === null ? 'missed' : `${heardIn}ms`))
			.join(', ');
		console.log(
			`announcement latencies over ${CYCLES} cycles: [${distribution}]`,
		);

		if (missed > MAX_MISSED) {
			throw new Error(
				`${missed} of ${CYCLES} hub announcements went unheard within ` +
					`${HEARD_MS / 1_000}s (at most ${MAX_MISSED} tolerated): [${distribution}]. ` +
					'A round or two missing is dropped multicast; losing them all means ' +
					'reception is broken rather than lossy. Check the boring cause first — ' +
					'that the phone still holds a wifi address on the same network as the host ' +
					'— then that the MulticastLock is still held.',
			);
		}
	});

	it('rediscovers the hub after a wifi bounce with the app in the foreground', async function () {
		// `on_resume` re-arms discovery, but it only fires on a foreground
		// transition — a network change under a visible app never reaches it, so
		// this is the one path with no re-arm behind it.
		if (phone.platform !== 'android') this.skip();

		const indicator = phone.groupChatPage.connectionStatusIndicator;

		// Restart the hub rather than just enabling it: a hub only announces when
		// it registers, so enabling an already-enabled one is a no-op the phone
		// never hears, and the precondition below would fail for the very reason
		// this suite exists.
		await host.offlinePage.setLocalMailboxEnabled(false);
		await waitForStatus(
			phone,
			indicator,
			'disconnected',
			TEARDOWN_MS,
			'client still saw a hub after the host stopped it, so the bounce has no clean baseline',
		);
		await host.offlinePage.setLocalMailboxEnabled(true);
		await waitForStatus(
			phone,
			indicator,
			'local',
			DISCOVERY_MS,
			'hub was not connected before the wifi bounce, so the bounce would prove nothing',
		);
		const addressBefore = await phone.wifiAddress();
		const addressAfter = await phone.cycleWifi(WIFI_DOWN_MS);
		const reconnectedAt = Date.now();

		// Re-enabling wifi lets the supplicant pick whatever saved network scores
		// best, which need not be the hub's. Landing elsewhere makes the hub
		// genuinely unreachable and any timing taken after it meaningless.
		const subnet = (address: string) =>
			address.split('.').slice(0, 3).join('.');
		if (subnet(addressBefore) !== subnet(addressAfter)) {
			throw new Error(
				`device came back on ${addressAfter}, not the hub's network (was ${addressBefore}); ` +
					"re-run with only the hub's SSID in range",
			);
		}

		await waitForStatus(
			phone,
			indicator,
			'local',
			DISCOVERY_MS,
			`hub on the same subnet was still undiscovered ${DISCOVERY_MS / 1_000}s after wifi came ` +
				'back, with the app foregrounded the whole time. mdns-sd queries once on a new ' +
				'address and does not retry promptly, so a dropped reply leaves the periodic ' +
				're-browse as the only recovery — the field report measured 6m40s here.',
		);
		console.log(
			`hub rediscovered ${Math.round((Date.now() - reconnectedAt) / 1000)}s after wifi returned`,
		);
	});
});
