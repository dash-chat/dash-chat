/**
 * Unified e2e config. The PLATFORMS env var lists the agents to launch as an
 * unordered multiset of platforms (default `desktop,desktop`) — `desktop`
 * (tauri-driver against the built binary), `android` (physical device via
 * Appium), `android-emulator` (running emulator via Appium), or `ios`
 * (connected iPhone via Appium/XCUITest) — so any combo runs through this one
 * config, e.g. `PLATFORMS=ios,ios just e2e run send-messages`. Combos are bound
 * by host OS, though: `desktop` needs Linux (tauri-driver/WebKitGTK), `ios` needs
 * macOS + a device, so they can't share one host.
 */
import type { ChildProcess } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { UI_TIMEOUT } from './helpers/timeouts';
import { killLeftoverMailboxServers } from './setup/cleanup';
import {
	buildMailboxServer,
	startLocalMailboxServer,
} from './setup/mailbox-server';
import { type AndroidKind, AndroidPlatform } from './setup/platforms/android';
import { DesktopPlatform } from './setup/platforms/desktop';
import { IosPlatform } from './setup/platforms/ios';
import type { AgentPlatform } from './setup/platforms/platform';
import {
	buildPushServer,
	pushTestingEnabled,
	startLocalPushServer,
} from './setup/push-server';
import {
	type AgentPlatformName,
	getSpecFileRetries,
	platformNames,
	remoteMailboxUrl,
} from './setup/test-env';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const nameBySlot = new Map<number, AgentPlatformName>(
	platformNames().map((name, i) => [i + 1, name]),
);

const desktopSlots = [...nameBySlot]
	.filter(([, name]) => name === 'desktop')
	.map(([slot]) => slot);
const iosSlots = [...nameBySlot]
	.filter(([, name]) => name === 'ios')
	.map(([slot]) => slot);
const androidKinds = new Map<number, AndroidKind>(
	[...nameBySlot].filter(
		(entry): entry is [number, AndroidKind] =>
			entry[1] !== 'desktop' && entry[1] !== 'ios',
	),
);

/** The host mailbox-server build, kicked off before the Android platform is
 * constructed so it overlaps the emulator boots that block construction.
 * Awaited in onPrepare. */
const mailboxBuild =
	process.env.WDIO_WORKER_ID === undefined && remoteMailboxUrl() === null
		? buildMailboxServer()
		: null;
// Register a handler now so a build failure before onPrepare awaits the
// promise doesn't crash node with an unhandled rejection.
mailboxBuild?.catch(() => {});

/** The push-notifications-server build, only when FCM_SERVICE_ACCOUNT_KEY is set
 * (the real-device push spec). Gated on the env var (not the throwing
 * pushServiceAccountKey()) so a bad path surfaces in onPrepare, not at load. */
const pushEnabled =
	process.env.WDIO_WORKER_ID === undefined && pushTestingEnabled();
const pushServerBuild = pushEnabled ? buildPushServer() : null;
pushServerBuild?.catch(() => {});

const android =
	androidKinds.size > 0 ? new AndroidPlatform(androidKinds) : null;
const ios = iosSlots.length > 0 ? new IosPlatform(iosSlots) : null;
const desktop =
	desktopSlots.length > 0 ? new DesktopPlatform(desktopSlots) : null;
const platforms: AgentPlatform[] = [];
if (desktop !== null) platforms.push(desktop);
if (android !== null) platforms.push(android);
if (ios !== null) platforms.push(ios);

// Android and iOS both drive their devices through Appium (uiautomator2 /
// xcuitest); they share one server on the same pinned port.
const appiumPort = android?.appiumPort ?? ios?.appiumPort ?? null;

function agentEntry(slot: number) {
	const platform = platforms.find(p => p.slots.includes(slot))!;
	return platform.remoteOptions(slot);
}

let mailboxServer: ChildProcess | undefined;
let mailboxLogger: ChildProcess | undefined;
let pushServer: ChildProcess | undefined;
let pushLogger: ChildProcess | undefined;

async function teardown() {
	for (const server of [mailboxServer, pushServer]) {
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
	mailboxLogger?.kill();
	pushLogger?.kill();
}

/** Save a per-agent screenshot of the current webview to .dbs/e2e/failures/. */
async function saveFailureScreenshots(test: {
	parent: string;
	title: string;
}): Promise<void> {
	const dir = path.join(ROOT, '.dbs', 'e2e', 'failures');
	mkdirSync(dir, { recursive: true });
	const slug = `${test.parent} ${test.title}`
		.replace(/[^a-zA-Z0-9]+/g, '-')
		.slice(0, 80);
	for (const name of browser.instances) {
		try {
			await browser
				.getInstance(name)
				.saveScreenshot(path.join(dir, `${slug}-${name}.png`));
		} catch {
			/* session may already be dead */
		}
	}
}

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: Object.fromEntries(
		[...nameBySlot.keys()].map(slot => [`agent${slot}`, agentEntry(slot)]),
	),

	services:
		appiumPort !== null ? [['appium', { args: { port: appiumPort } }]] : [],

	logLevel: 'warn',
	waitforTimeout: UI_TIMEOUT,
	// Android session creation installs the APK and boots UiAutomator2 — slow.
	connectionRetryTimeout: 300_000,

	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 120_000,
	},

	reporters: ['spec'],

	async onPrepare() {
		// A failed onPrepare must abort the run: wdio only logs hook errors and
		// would carry on into sessions doomed to hang out their timeouts.
		try {
			// Clean up leftover databases from previous interrupted runs
			const dataDir = path.join(ROOT, '.dbs', 'e2e');
			try {
				rmSync(dataDir, { recursive: true, force: true });
			} catch {
				// ignore
			}

			killLeftoverMailboxServers();

			const mailboxInfoPath = path.join(
				ROOT,
				'.dbs',
				'e2e',
				'mailbox-info.json',
			);

			// When MAILBOX_URL names an allowlisted deployment environment, run
			// against its cloud mailbox instead of spawning a local server. Specs
			// that drive the mailbox's lifecycle skip themselves via
			// isRemoteMailbox().
			const remoteUrl = remoteMailboxUrl();
			let mailboxPort: number | null = null;
			let pushPort: number | null = null;
			if (remoteUrl !== null) {
				process.env.MAILBOX_URL = remoteUrl;
				mkdirSync(path.dirname(mailboxInfoPath), { recursive: true });
				writeFileSync(
					mailboxInfoPath,
					JSON.stringify({ remote: true, url: remoteUrl }),
				);
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
				// Start a local mailbox server so e2e tests don't hit the internet.
				({
					proc: mailboxServer,
					logger: mailboxLogger,
					port: mailboxPort,
				} = await startLocalMailboxServer(pushUrl));
			}

			for (const platform of platforms) {
				await platform.onPrepare({ mailboxPort, pushPort });
			}
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

	/** On failure, save a per-agent screenshot to .dbs/e2e/failures/ so flakes
	 * that only reproduce on slow devices leave usable evidence behind. */
	async afterTest(test, _context, result) {
		if (!result.passed) await saveFailureScreenshots(test);
	},

	async afterHook(test, _context, result) {
		if (!result.passed) await saveFailureScreenshots(test);
	},

	async afterSession() {
		for (const platform of platforms) {
			await platform.afterSession();
		}
	},

	async onComplete() {
		await teardown();
	},
};
