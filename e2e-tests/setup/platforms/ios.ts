import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { networkInterfaces } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { syncXcodeEnv } from '../../../scripts/sync-xcode-env';
import { echoLinesWithPrefix } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { cleanBuildEnv } from '../build-env';
import type { AgentPlatform, PrepareContext } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');
const E2E_DIR = path.resolve(__dirname, '..', '..');

const APP_BUNDLE_ID = 'studio.darksoil.dashchat';
const IOS_BUILD_DIR = path.join(ROOT, 'src-tauri/gen/apple/build');
const APPIUM_BIN = path.join(E2E_DIR, 'node_modules', '.bin', 'appium');

// The app project signs with Automatic signing under this team (see
// src-tauri/gen/apple/dash-chat.xcodeproj); WebDriverAgent reuses it so Appium
// can build and install WDA on the physical device.
const DEV_TEAM = '4XN3VLHC68';
const WDA_BUNDLE_ID = 'studio.darksoil.dashchat.WebDriverAgentRunner';

function assertIosToolsAvailable() {
	for (const tool of ['idevice_id', 'xcrun']) {
		try {
			execSync(`command -v ${tool}`, { stdio: 'ignore' });
		} catch {
			throw new Error(
				`${tool} not found — iOS agents run in the plain shell and need ` +
					'system Xcode plus libimobiledevice (brew install libimobiledevice)',
			);
		}
	}
}

// Host IPv4 the device reaches the mailbox at
function detectHostIp(): string {
	const override = process.env.E2E_HOST_IP;
	if (override !== undefined && override !== '') return override;
	const ifaces = networkInterfaces();
	const candidates = ['en0', ...Object.keys(ifaces)];
	for (const name of candidates) {
		for (const addr of ifaces[name] ?? []) {
			if (addr.family === 'IPv4' && !addr.internal) return addr.address;
		}
	}
	throw new Error(
		'Could not detect a host LAN IP for the iOS device to reach the mailbox — ' +
			'set E2E_HOST_IP',
	);
}

function connectedDevices(): string[] {
	return execSync('idevice_id -l', { encoding: 'utf8' })
		.split('\n')
		.map(line => line.trim())
		.filter(Boolean);
}

// Claim one connected iPhone per slot
function claimDevices(slots: number[]): Map<number, string> {
	const pool = connectedDevices();
	const udids = new Map<number, string>();
	for (const slot of slots) {
		const pinned =
			process.env[`_WDIO_IOS_UDID${slot}`] ?? process.env[`IOS_UDID${slot}`];
		if (pinned !== undefined) {
			udids.set(slot, pinned);
			process.env[`_WDIO_IOS_UDID${slot}`] = pinned;
			const i = pool.indexOf(pinned);
			if (i !== -1) pool.splice(i, 1);
			continue;
		}
		const udid = pool.shift();
		if (udid === undefined) {
			throw new Error(
				`Not enough connected iPhones for agent${slot} ` +
					`(connected: ${connectedDevices().join(', ') || 'none'}). Connect a ` +
					`trusted device, or set IOS_UDID${slot}.`,
			);
		}
		process.env[`_WDIO_IOS_UDID${slot}`] = udid;
		udids.set(slot, udid);
	}
	return udids;
}

/** The xcuitest driver lives in APPIUM_HOME (not node_modules); install it on
 *  first run. */
function ensureXcuitestDriver() {
	const installed = execSync(
		`"${APPIUM_BIN}" driver list --installed 2>&1 || true`,
		{ encoding: 'utf8', cwd: E2E_DIR },
	);
	if (!installed.includes('xcuitest')) {
		// Pinned to the last version compatible with the appium 2.x server (8.x
		// requires appium 3.x), like the android uiautomator2 pin.
		execSync(`"${APPIUM_BIN}" driver install xcuitest@7.35.1`, {
			stdio: 'inherit',
			cwd: E2E_DIR,
		});
	}
}

// Locate the .ipa produced by `tauri ios build`
function builtIpa(): string {
	const out = execSync(`find "${IOS_BUILD_DIR}" -maxdepth 3 -name '*.ipa'`, {
		encoding: 'utf8',
	}).trim();
	const ipa = out.split('\n').filter(Boolean)[0];
	if (ipa === undefined) {
		throw new Error(
			`No .ipa found under ${IOS_BUILD_DIR} after 'tauri ios build'`,
		);
	}
	return ipa;
}

// Tail the app's device console and echo it with an agent prefix
function startSyslogLogger(agent: string, udid: string): ChildProcess | null {
	try {
		const proc = spawn(
			'idevicesyslog',
			['-u', udid, '--process', 'Dash Chat'],
			{ stdio: ['ignore', 'pipe', 'ignore'] },
		);
		proc.on('error', () => {});
		if (proc.stdout) echoLinesWithPrefix(agent, proc.stdout);
		return proc;
	} catch {
		return null;
	}
}

/**
 * Agents running the e2e app on physical iPhones through Appium (XCUITest)
 * sessions that land directly in the app's WKWebView context, so specs and page
 * objects work exactly as on desktop and Android.
 */
export class IosPlatform implements AgentPlatform {
	readonly slots: number[];
	readonly appiumPort: number;
	private udids: Map<number, string>;
	private ipaPath: string | undefined;
	private loggers = new Map<number, ChildProcess>();

	constructor(slots: number[]) {
		assertIosToolsAvailable();
		this.slots = slots;
		// Same pinned var as AndroidPlatform: a mixed run shares one Appium
		// server hosting both drivers.
		this.appiumPort = allocatePinnedPort('_WDIO_APPIUM_PORT');
		process.env.APPIUM_HOME = path.join(E2E_DIR, '.appium');
		// Workers reload this module; device provisioning belongs to the
		// launcher, which claims before any worker starts.
		this.udids = claimDevices(slots);
	}

	remoteOptions(slot: number) {
		const udid = this.udids.get(slot)!;
		return {
			port: this.appiumPort,
			capabilities: {
				platformName: 'iOS',
				'appium:automationName': 'XCUITest',
				'appium:udid': udid,
				// ipaPath is set in onPrepare (launcher); worker config loads read
				// the same build output, so recompute it lazily if unset.
				'appium:app': this.ipaPath ?? builtIpa(),
				'appium:bundleId': APP_BUNDLE_ID,
				'appium:autoWebview': true,
				'appium:autoWebviewTimeout': 30_000,
				'appium:webviewConnectTimeout': 30_000,
				// Real native taps for webview clicks: JS-synthesized clicks don't
				// reliably fire tap handlers on Konsta list items in WKWebView, so
				// navigations silently no-op without this.
				'appium:nativeWebTap': true,
				// Auto-accept native permission dialogs (e.g. the notifications
				// prompt) so they don't sit over the webview and block interaction —
				// the iOS analog of android's autoGrantPermissions.
				'appium:autoAcceptAlerts': true,
				// WDA build + signing under the app's team.
				'appium:xcodeOrgId': DEV_TEAM,
				'appium:xcodeSigningId': 'Apple Development',
				'appium:updatedWDABundleId': WDA_BUNDLE_ID,
				'appium:derivedDataPath': path.join(E2E_DIR, '.appium', 'wda'),
				'appium:wdaLaunchTimeout': 120_000,
				'appium:newCommandTimeout': 240,
				// Surface the WDA xcodebuild output so signing/config failures are
				// diagnosable instead of a bare "xcodebuild failed with code 65".
				'appium:showXcodeLog': true,
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare(ctx: PrepareContext) {
		if (ctx.mailboxPort === null) {
			throw new Error(
				'iOS agents need a local mailbox server (the e2e build bakes ' +
					'http://<host-ip>:<port>)',
			);
		}

		ensureXcuitestDriver();

		// Bake the LAN-reachable mailbox URL into the build
		const mailboxUrl = `http://${detectHostIp()}:${ctx.mailboxPort}`;
		syncXcodeEnv({ MAILBOX_URL: mailboxUrl });
		execSync('pnpm tauri ios build --debug --features e2e-tests', {
			cwd: ROOT,
			stdio: 'inherit',
			// cleanBuildEnv lets pnpm run as it does from a plain shell (see there).
			env: cleanBuildEnv({
				MAILBOX_URL: mailboxUrl,
				VITE_E2E: 'true',
				IPHONEOS_DEPLOYMENT_TARGET: '17.0',
				// Drop debuginfo (a full-debug iOS build is >10GB and fills the disk;
				// this is compile-time, so it can't break signing). Unlike android,
				// do NOT also STRIP symbols post-build — that leaves the disk/temp
				// churn that made the .ipa export's codesign fail.
				CARGO_PROFILE_DEV_DEBUG: '0',
			}),
		});

		this.ipaPath = builtIpa();

		// Uninstall any prior install so the session installs the fresh build
		// instead of failing on a signature or version mismatch
		for (const udid of this.udids.values()) {
			try {
				execSync(
					`xcrun devicectl device uninstall app --device ${udid} ${APP_BUNDLE_ID}`,
					{ stdio: 'ignore' },
				);
			} catch {
				/* not installed */
			}
		}
	}

	async beforeSession() {
		for (const [slot, udid] of this.udids) {
			this.loggers.get(slot)?.kill();
			const logger = startSyslogLogger(`agent-${slot}`, udid);
			if (logger) this.loggers.set(slot, logger);
		}
	}

	async afterSession() {
		for (const logger of this.loggers.values()) logger.kill();
		this.loggers.clear();
	}

	async onComplete() {
		for (const logger of this.loggers.values()) logger.kill();
	}
}
