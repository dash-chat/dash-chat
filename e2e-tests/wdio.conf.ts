/**
 * Unified e2e config. The AGENT_1/AGENT_2 env vars pick each agent's platform
 * — `desktop` (default, tauri-driver against the built binary), `android`
 * (physical device via Appium) or `android-emulator` (running emulator via
 * Appium) — so any combo runs through this one config, e.g.
 * `AGENT_1=android just test e2e run send-messages` for phone + desktop.
 */
import { type ChildProcess } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { UI_TIMEOUT } from './helpers/timeouts';
import { killLeftoverMailboxServers } from './setup/cleanup';
import { startLocalMailboxServer } from './setup/mailbox-server';
import { AndroidPlatform, type AndroidKind } from './setup/platforms/android';
import { DesktopPlatform } from './setup/platforms/desktop';
import type { AgentPlatform } from './setup/platforms/platform';
import {
	agentPlatform,
	type AgentPlatformName,
	getSpecFileRetries,
	remoteMailboxUrl,
} from './setup/test-env';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const nameBySlot = new Map<number, AgentPlatformName>(
	[1, 2].map(slot => [slot, agentPlatform(slot)]),
);

const desktopSlots = [...nameBySlot]
	.filter(([, name]) => name === 'desktop')
	.map(([slot]) => slot);
const androidKinds = new Map<number, AndroidKind>(
	[...nameBySlot].filter(
		(entry): entry is [number, AndroidKind] => entry[1] !== 'desktop',
	),
);

const android = androidKinds.size > 0 ? new AndroidPlatform(androidKinds) : null;
const desktop =
	desktopSlots.length > 0 ? new DesktopPlatform(desktopSlots) : null;
const platforms: AgentPlatform[] = [];
if (desktop !== null) platforms.push(desktop);
if (android !== null) platforms.push(android);

function agentEntry(slot: number) {
	const platform = platforms.find(p => p.slots.includes(slot))!;
	return platform.remoteOptions(slot);
}

// Only specs proven to work on-device. Grow this list as specs are ported.
const ON_DEVICE_SPECS = ['./specs/send-messages.spec.ts'];

let mailboxServer: ChildProcess | undefined;
let mailboxLogger: ChildProcess | undefined;

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: android !== null ? ON_DEVICE_SPECS : ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: {
		agent1: agentEntry(1),
		agent2: agentEntry(2),
	},

	services:
		android !== null
			? [['appium', { args: { port: android.appiumPort } }]]
			: [],

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
			if (remoteUrl !== null) {
				process.env.MAILBOX_URL = remoteUrl;
				mkdirSync(path.dirname(mailboxInfoPath), { recursive: true });
				writeFileSync(
					mailboxInfoPath,
					JSON.stringify({ remote: true, url: remoteUrl }),
				);
				console.log(`Using remote mailbox at ${remoteUrl}`);
			} else {
				// Start a local mailbox server so e2e tests don't hit the internet.
				({
					proc: mailboxServer,
					logger: mailboxLogger,
					port: mailboxPort,
				} = await startLocalMailboxServer());
			}

			for (const platform of platforms) {
				await platform.onPrepare({ mailboxPort });
			}
		} catch (err) {
			console.error('onPrepare failed, aborting run:', err);
			process.exit(1);
		}
	},

	async beforeSession() {
		for (const platform of platforms) {
			await platform.beforeSession();
		}
	},

	async afterSession() {
		for (const platform of platforms) {
			await platform.afterSession();
		}
	},

	onComplete() {
		if (mailboxServer?.pid) {
			// Negative PID = signal the entire process group the detached
			// mailbox server runs in.
			try {
				process.kill(-mailboxServer.pid, 'SIGTERM');
			} catch {
				/* already gone */
			}
		}
		for (const platform of platforms) {
			platform.onComplete();
		}
		mailboxLogger?.kill();
	},
};
