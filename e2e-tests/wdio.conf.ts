/**
 * Unified e2e config. Every agent a spec asks for that no phone fills is a
 * desktop agent — the host's own build, driven through its embedded WebDriver
 * server — launched for that spec file. The PHONES env var lists the phones
 * the run drives as an unordered multiset — `android` (physical device via
 * Appium), `android-emulator` (running emulator via Appium), or `ios`
 * (connected iPhone via Appium/XCUITest) — e.g.
 * `PHONES=ios,ios just e2e run send-messages`; `ios` needs macOS.
 */
import { setOptions } from 'expect-webdriverio';
import type { ChildProcess } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { AndroidNotifications } from './helpers/components/notifications/android';
import { IosNotifications } from './helpers/components/notifications/ios';
import { notificationMatchers } from './helpers/components/notifications/matchers';
import { RENDER_SETTLE_WINDOW, UI_TIMEOUT } from './helpers/timeouts';
import { startAppium } from './setup/appium-server';
import { claimAllWhenFreeSync, release } from './setup/claims';
import {
	killAllE2EProcesses,
	killLeftoverLocalHubs,
	killLeftoverMailboxServers,
} from './setup/cleanup';
import {
	failureSlug,
	failuresDir,
	saveFailureScreenshot,
} from './setup/failure-screenshots';
import { releaseWifiDevice } from './setup/host-wifi';
import { LOCAL_HUB_PACKAGE } from './setup/local-hub';
import { ensureLoopback } from './setup/loopback';
import { ensureHealthyMailbox } from './setup/mailbox-control';
import {
	buildCargoPackages,
	startLocalMailboxServer,
} from './setup/mailbox-server';
import { CHECKOUT_CLAIM } from './setup/network-id';
import { type AndroidKind, AndroidPlatform } from './setup/platforms/android';
import { buildDesktopApp, stopDesktopAgents } from './setup/platforms/desktop';
import { IosPlatform, clearIosAppData } from './setup/platforms/ios';
import type { AgentPlatform } from './setup/platforms/platform';
import {
	buildPushServer,
	pushTestingEnabled,
	startLocalPushServer,
} from './setup/push-server';
import { specAgents } from './setup/setup-agents';
import {
	type PhonePlatformName,
	getSpecFileRetries,
	phonePlatforms,
	remoteMailboxUrl,
} from './setup/test-env';
import { startToxiproxy } from './setup/toxiproxy';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const nameBySlot = new Map<number, PhonePlatformName>(
	phonePlatforms().map((name, i) => [i + 1, name]),
);

const iosSlots = [...nameBySlot]
	.filter(([, name]) => name === 'ios')
	.map(([slot]) => slot);
const androidKinds = new Map<number, AndroidKind>(
	[...nameBySlot].filter(
		(entry): entry is [number, AndroidKind] => entry[1] !== 'ios',
	),
);

// Before anything is built, killed or claimed: the launcher waits for any run
// already going in this checkout — its data dir and baked network id are one
// per checkout. Workers inherit the launcher's claim.
if (process.env.WDIO_WORKER_ID === undefined) {
	claimAllWhenFreeSync([{ candidates: [CHECKOUT_CLAIM], needed: 1 }]);
}

/** The host mailbox-server build — plus the standalone hub the discovery
 * specs spawn — kicked off before the Android platform is constructed so it
 * overlaps the emulator boots that block construction. Awaited in onPrepare. */
const mailboxBuild =
	process.env.WDIO_WORKER_ID === undefined && remoteMailboxUrl() === null
		? buildCargoPackages(['mailbox-server', LOCAL_HUB_PACKAGE])
		: null;
// Register a handler now so a build failure before onPrepare awaits the
// promise doesn't crash node with an unhandled rejection.
mailboxBuild?.catch(() => {});

/** The push-notifications-server build, only when a service-account key is
 * present and a mobile agent runs (the real-device push spec). */
const pushEnabled =
	process.env.WDIO_WORKER_ID === undefined && pushTestingEnabled();
const pushServerBuild = pushEnabled ? buildPushServer() : null;
pushServerBuild?.catch(() => {});

const android =
	androidKinds.size > 0 ? new AndroidPlatform(androidKinds) : null;
const ios = iosSlots.length > 0 ? new IosPlatform(iosSlots) : null;
const platforms: AgentPlatform[] = [];
if (android !== null) platforms.push(android);
if (ios !== null) platforms.push(ios);

// Android and iOS both drive their devices through Appium (uiautomator2 /
// xcuitest); they share one server on the same pinned port.
const appiumPort = android?.appiumPort ?? ios?.appiumPort ?? null;

function agentEntry(slot: number) {
	const platform = platforms.find(p => p.slots.includes(slot))!;
	return platform.remoteOptions(slot);
}

/** The run's fixed sessions: one per phone. wdio refuses a run with none, so
 *  a phone-less run gets one on webdriverio's protocol stub, which opens no
 *  session and drives nothing. */
function sessionEntries() {
	if (nameBySlot.size === 0) {
		return {
			host: { automationProtocol: './protocol-stub.js', capabilities: {} },
		};
	}
	return Object.fromEntries(
		[...nameBySlot.keys()].map(slot => [`agent${slot}`, agentEntry(slot)]),
	);
}

let mailboxServer: ChildProcess | undefined;
let mailboxLogger: ChildProcess | undefined;
let pushServer: ChildProcess | undefined;
let pushLogger: ChildProcess | undefined;
let toxiproxy: ChildProcess | undefined;
let toxiproxyLogger: ChildProcess | undefined;
let appium: ChildProcess | undefined;
let mailboxTls: ChildProcess | undefined;
let mailboxTlsLogger: ChildProcess | undefined;

async function teardown() {
	for (const server of [
		mailboxServer,
		mailboxTls,
		pushServer,
		toxiproxy,
		appium,
	]) {
		if (server?.pid) {
			// Negative PID = signal the entire detached process group.
			try {
				process.kill(-server.pid, 'SIGTERM');
			} catch {
				/* already gone */
			}
		}
	}
	for (const platform of platforms) {
		await platform.onComplete();
	}
	killAllE2EProcesses();
	mailboxLogger?.kill();
	pushLogger?.kill();
	toxiproxyLogger?.kill();
	mailboxTlsLogger?.kill();
	release(CHECKOUT_CLAIM);
}

/** Save a per-agent screenshot of the current webview to .dbs/e2e/failures/. */
async function saveFailureScreenshots(test: {
	parent: string;
	title: string;
}): Promise<void> {
	const dir = failuresDir();
	const slug = failureSlug(`${test.parent} ${test.title}`);
	for (const [i, agent] of specAgents.entries()) {
		await saveFailureScreenshot(
			agent,
			path.join(dir, `${slug}-agent${i + 1}.png`),
		);
	}
}

/** After a failure, put every phone whose session was left outside the app's
 *  webview — on a notification shade, in Settings — back in it: otherwise
 *  every test after it fails on an unrelated-looking selector. A phone still
 *  in the webview is left alone, since the way back from a shade (Back on
 *  Android) would move the app itself. */
async function recoverPhones(): Promise<void> {
	for (const slot of [...androidKinds.keys(), ...iosSlots]) {
		const b = browser.getInstance(`agent${slot}`);
		try {
			const context = await b.getContext();
			const id = typeof context === 'string' ? context : context.id;
			if (id.startsWith('WEBVIEW')) continue;
		} catch {
			// Where the session is cannot be told, and a guess could press
			// Back on the app itself.
			continue;
		}
		const helper = iosSlots.includes(slot)
			? new IosNotifications(b)
			: new AndroidNotifications(b);
		await helper.recover();
	}
}

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: sessionEntries(),

	logLevel: 'warn',
	waitforTimeout: UI_TIMEOUT,
	// Android session creation installs the APK and boots UiAutomator2 — slow.
	connectionRetryTimeout: 300_000,

	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		// Phones and emulators can spend minutes on app cold starts and p2p
		// syncs that desktop finishes in seconds, so give their hooks and
		// tests more headroom.
		timeout: android !== null || ios !== null ? 300_000 : 120_000,
		// Fails any test during which an agent hit an uncaught error.
		require: [path.join(__dirname, 'setup', 'fail-on-uncaught-errors.ts')],
	},

	// The spec reporter writes the assertion that failed to stdout and nowhere
	// else, so a finished run leaves only screenshots to reconstruct it from.
	// The json one keeps each test's error and stack next to them on disk.
	reporters: [
		'spec',
		['json', { outputDir: path.join(ROOT, '.dbs', 'e2e', 'results') }],
	],

	async onPrepare() {
		// A failed onPrepare must abort the run: wdio only logs hook errors and
		// would carry on into sessions doomed to hang out their timeouts.
		try {
			ensureLoopback();
			// Before the wipe: anything still running holds handles under
			// `.dbs/e2e` and goes on writing, which leaves the dir dirty behind
			// the `rmSync`.
			killLeftoverMailboxServers();
			// A desktop run clears its own app processes, but a leftover agent
			// outlives a phone-only run, where nothing used to clear it: it goes
			// on announcing itself over mDNS under the run's own network id, and
			// every agent that finds it spends a discovery session timing out on
			// a node that will never answer.
			killAllE2EProcesses();

			// Clean up leftover databases from previous interrupted runs
			const dataDir = path.join(ROOT, '.dbs', 'e2e');
			try {
				rmSync(dataDir, { recursive: true, force: true });
			} catch {
				// ignore
			}
			// Appium writes its log here, so the directory has to exist before
			// the first server does.
			mkdirSync(dataDir, { recursive: true });

			// When MAILBOX_URL names a deployment environment, run against its
			// cloud mailbox instead of spawning a local server. Specs that drive
			// the mailbox's lifecycle skip themselves via isRemoteMailbox().
			const remoteUrl = remoteMailboxUrl();
			let mailboxPort: number | null = null;
			let pushPort: number | null = null;
			if (remoteUrl !== null) {
				console.log(`Using remote mailbox at ${remoteUrl}`);
			} else {
				// Real-device push tests: start the push-notifications server first
				// so the mailbox can be spawned pointing at it. Only meaningful with
				// a local mailbox — a remote cloud mailbox can't reach our host.
				let pushUrl: string | undefined;
				if (pushEnabled) {
					await pushServerBuild;
					const push = await startLocalPushServer();
					if (push !== null) {
						({ proc: pushServer, logger: pushLogger, port: pushPort } = push);
						pushUrl = push.url;
					}
				}

				await mailboxBuild;
				// The mailbox's public port is a link through this, for the
				// specs that degrade it.
				({ proc: toxiproxy, logger: toxiproxyLogger } = await startToxiproxy());
				// Start a local mailbox server so e2e tests don't hit the internet.
				({
					proc: mailboxServer,
					logger: mailboxLogger,
					tls: mailboxTls,
					tlsLogger: mailboxTlsLogger,
					port: mailboxPort,
				} = await startLocalMailboxServer(pushUrl));
			}

			buildDesktopApp();

			for (const platform of platforms) {
				await platform.onPrepare({ mailboxPort, pushPort });
			}
			if (appiumPort !== null) appium = await startAppium(appiumPort);
		} catch (err) {
			console.error('onPrepare failed, aborting run:', err);
			// process.exit skips onComplete — tear down the already-started
			// mailbox server (and any platform state) so it isn't orphaned.
			try {
				await teardown();
			} catch {
				/* best effort */
			}
			process.exit(1);
		}
	},

	async beforeSession() {
		for (const platform of platforms) {
			await platform.beforeSession();
		}
	},

	/** Negated expect matchers (`.not.toBeExisting()`, …) poll their full wait
	 * before passing, so the 30s waitforTimeout default turns every absence
	 * assertion into a 30s stall. Cap all matchers at the settle window; an
	 * assertion that genuinely needs longer opts in with `{ wait: UI_TIMEOUT }`. */
	async before() {
		setOptions({ wait: RENDER_SETTLE_WINDOW });
		expect.extend(notificationMatchers);
		killLeftoverLocalHubs();
		await ensureHealthyMailbox();
	},

	/** On failure, save a per-agent screenshot to .dbs/e2e/failures/ so flakes
	 * that only reproduce on slow devices leave usable evidence behind. A skip
	 * is reported as `passed: false` with no error (wdio sets
	 * `passed: !error && !skip`), so only an error counts as a failure — a spec
	 * that skips itself for the launched platforms must not leave one behind. */
	async afterTest(test, _context, result) {
		if (result.error === undefined) return;
		await saveFailureScreenshots(test);
		await recoverPhones();
	},

	async afterHook(test, _context, result) {
		if (result.error === undefined) return;
		await saveFailureScreenshots(test);
		await recoverPhones();
	},

	async after() {
		for (const slot of iosSlots) {
			await clearIosAppData(browser.getInstance(`agent${slot}`));
		}
		killLeftoverLocalHubs();
		await ensureHealthyMailbox();
	},

	async afterSession() {
		releaseWifiDevice();
		await stopDesktopAgents();
		for (const platform of platforms) {
			await platform.afterSession();
		}
	},

	async onComplete() {
		await teardown();
	},
};
