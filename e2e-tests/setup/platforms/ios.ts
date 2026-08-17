import { type ChildProcess, execSync, spawn } from 'node:child_process';
import {
	constants,
	copyFileSync,
	existsSync,
	mkdirSync,
	statSync,
} from 'node:fs';
import { networkInterfaces } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { syncXcodeEnv } from '../../../scripts/sync-xcode-env';
import { echoLinesWithPrefix } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { envWithoutWdioLoader } from '../harness-env';
import type { AgentPlatform, PrepareContext } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');
const E2E_DIR = path.resolve(__dirname, '..', '..');

const APP_BUNDLE_ID = 'studio.darksoil.dashchat';
const IOS_BUILD_DIR = path.join(ROOT, 'src-tauri/gen/apple/build');
const APPIUM_BIN = path.join(E2E_DIR, 'node_modules', '.bin', 'appium');
// Fixed home for the .ipa the sessions install, copied here by onPrepare. The
// capabilities `appium:app` reads are resolved when wdio.conf.ts loads — before
// onPrepare builds anything — so it can't name a path under the build dir,
// whose layout the tauri build rearranges (it exports the .ipa to the build
// dir, then moves it into an arch subdir).
const SESSION_IPA = path.join(E2E_DIR, '.appium', 'dash-chat-e2e.ipa');

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

/** Whether the device could route to this address over the LAN. 169.254 is
 *  excluded on purpose: a tethered iPhone shows up as a USB network interface
 *  with a self-assigned address in that range, and baking one of those leaves
 *  the mailbox unreachable from every device. */
function isPrivateLanAddress(address: string): boolean {
	const [a, b] = address.split('.').map(Number);
	return a === 10 || a === 192 || (a === 172 && b >= 16 && b <= 31);
}

/** Built-in wifi/ethernet (en0, en1, …) ahead of USB and virtual interfaces,
 *  lowest number first. A tethered iPhone sharing its connection hands out a
 *  private address too, and it must not win over the real LAN. */
function interfaceRank(name: string): number {
	const en = /^en(\d+)$/.exec(name);
	return en === null ? 1000 : Number(en[1]);
}

/** Host IPv4 the device reaches the mailbox at. Baked into the build, so
 *  picking a wrong-but-plausible one costs a whole run: every cross-device
 *  assertion times out with the app quietly showing its offline state. */
function detectHostIp(): string {
	const override = process.env.E2E_HOST_IP;
	if (override !== undefined && override !== '') return override;
	const found = Object.entries(networkInterfaces())
		.flatMap(([name, addrs]) => (addrs ?? []).map(addr => ({ name, addr })))
		.filter(({ addr }) => addr.family === 'IPv4' && !addr.internal);
	const usable = found
		.filter(({ addr }) => isPrivateLanAddress(addr.address))
		.sort((a, b) => interfaceRank(a.name) - interfaceRank(b.name));
	if (usable.length === 0) {
		const seen = found.map(f => `${f.name}=${f.addr.address}`).join(', ');
		throw new Error(
			'No private LAN IPv4 for the iOS devices to reach the mailbox on — ' +
				`set E2E_HOST_IP. Non-internal IPv4 seen: ${seen || 'none'}`,
		);
	}
	const { name, addr } = usable[0];
	console.log(`[ios] baking mailbox host ip ${addr.address} (${name})`);
	return addr.address;
}

/** Fail the moment the address baked into the app stops being ours.
 *
 *  The mailbox url is fixed at build time, so a DHCP lease moving the host mid
 *  run leaves every device pointed at an address nobody answers on. Nothing
 *  surfaces that: the apps just show their offline state and each cross-device
 *  wait burns its full timeout, so a run can rot for half an hour and the
 *  failures all look like unrelated sync bugs. */
function assertBakedHostIpIsStillOurs(): void {
	const baked = process.env._WDIO_IOS_HOST_IP;
	if (baked === undefined) return;
	const current = Object.values(networkInterfaces())
		.flatMap(addrs => addrs ?? [])
		.filter(addr => addr.family === 'IPv4' && !addr.internal)
		.map(addr => addr.address);
	if (current.includes(baked)) return;
	throw new Error(
		`The mailbox url baked into the app points at ${baked}, which this host ` +
			`no longer holds (now: ${current.join(', ') || 'no LAN address'}). The ` +
			'devices cannot reach the mailbox, so every cross-device wait would ' +
			'time out. Rebuild, or pin E2E_HOST_IP and rerun.',
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

/** The freshest .ipa under the build dir. Newest wins because earlier layouts
 *  leave their .ipa behind when the build writes the next one elsewhere. */
function builtIpa(): string {
	const out = existsSync(IOS_BUILD_DIR)
		? execSync(`find "${IOS_BUILD_DIR}" -maxdepth 3 -name '*.ipa'`, {
				encoding: 'utf8',
			}).trim()
		: '';
	const ipas = out
		.split('\n')
		.filter(Boolean)
		.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
	if (ipas.length === 0) {
		throw new Error(
			`No .ipa found under ${IOS_BUILD_DIR} after 'tauri ios build'`,
		);
	}
	return ipas[0];
}

/** Put a device back to a freshly-installed app, retrying the install: CoreDevice
 * intermittently fails with a transient "unable to create bookmark data" /
 * "No such file" error even though the .ipa exists. Best-effort — a device left
 * without the app falls back to the session's own `appium:app` install. */
async function reinstallApp(udid: string): Promise<void> {
	try {
		execSync(
			`xcrun devicectl device uninstall app --device ${udid} ${APP_BUNDLE_ID}`,
			{ stdio: 'ignore' },
		);
	} catch {
		/* not installed */
	}
	for (let attempt = 1; attempt <= 3; attempt++) {
		try {
			execSync(
				`xcrun devicectl device install app --device ${udid} "${SESSION_IPA}"`,
				{ stdio: 'inherit' },
			);
			return;
		} catch (err) {
			if (attempt === 3) {
				console.warn(
					`[ios] devicectl install failed for ${udid} after ${attempt} ` +
						`attempts; falling back to the session's appium:app install: ${err}`,
				);
				return;
			}
			await new Promise(resolve => setTimeout(resolve, 2000));
		}
	}
}

/** Kill any tail still attached to `udid`.
 *
 *  Each spec runs in its own worker, so `afterSession`'s kill only ever reaches
 *  that worker's own tails and a worker that exits early leaves its own behind.
 *  The device relay serves one consumer, so a single survivor makes every later
 *  tail connect to a stream that stays empty — which is silent, and looks
 *  exactly like an app that logged nothing. */
function killStaleSyslogLoggers(udid: string): void {
	try {
		execSync(`pkill -f "idevicesyslog -u ${udid}"`, { stdio: 'ignore' });
	} catch {
		/* nothing was attached */
	}
}

/** Processes whose output the run cares about, in idevicesyslog's `a|b` form.
 *  The push extension is a separate process, so filtering to the app alone
 *  leaves every push failure undiagnosable. */
const LOGGED_PROCESSES = 'Dash Chat|PushNotificationsExtension';

// Tail the app's device console and echo it with an agent prefix
function startSyslogLogger(agent: string, udid: string): ChildProcess | null {
	killStaleSyslogLoggers(udid);
	try {
		const proc = spawn(
			'idevicesyslog',
			['-u', udid, '--process', LOGGED_PROCESSES],
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
				'appium:app': SESSION_IPA,
				'appium:bundleId': APP_BUNDLE_ID,
				// beforeSession already reinstalled this exact build through
				// devicectl; without this the driver installs it a second time
				// (`enforceAppInstall` defaults to reinstall-every-session), which is
				// the usbmux traffic that breaks session creation. Left as `false`
				// rather than dropping `appium:app`, so a device the devicectl install
				// failed on is still recovered by the session.
				'appium:enforceAppInstall': false,
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
				// Let xcodebuild register a not-yet-provisioned device on the dev
				// portal during the WDA build, instead of failing with 'provisioning
				// profile … doesn't include the currently selected device' — a freshly
				// connected iPhone isn't in the team profile yet.
				'appium:allowProvisioningDeviceRegistration': true,
				'appium:updatedWDABundleId': WDA_BUNDLE_ID,
				// Per-slot DerivedData: a two-device run (PLATFORMS=ios,ios) starts both
				// sessions at once, and two xcodebuilds sharing one DerivedData collide
				// (WDA "xcodebuild failed with code 65"). Same reason as the per-slot
				// ports above.
				'appium:derivedDataPath': path.join(E2E_DIR, '.appium', `wda-${slot}`),
				'appium:wdaLocalPort': allocatePinnedPort(`_WDIO_WDA_PORT${slot}`),
				'appium:mjpegServerPort': allocatePinnedPort(`_WDIO_MJPEG_PORT${slot}`),
				'appium:wdaLaunchTimeout': 120_000,
				// 0 disables idle expiry: specs like review-checks park one agent
				// for the whole spec after setup, far beyond any sane timeout.
				'appium:newCommandTimeout': 0,
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

		// Bake the LAN-reachable mailbox URL into the build, plus — for the
		// real-device push spec — the push-notifications server URL so the device
		// registers its FCM token with the host's local push server.
		const hostIp = detectHostIp();
		// Workers inherit the launcher's env, so this is what they check the
		// host still holds before each session.
		process.env._WDIO_IOS_HOST_IP = hostIp;
		const mailboxUrl = `http://${hostIp}:${ctx.mailboxPort}`;
		const bakedEnv: Record<string, string> = { MAILBOX_URL: mailboxUrl };
		if (ctx.pushPort !== null) {
			bakedEnv.PUSH_NOTIFICATIONS_SERVER_URL = `http://${hostIp}:${ctx.pushPort}`;
		}
		syncXcodeEnv(bakedEnv);
		execSync('pnpm tauri ios build --debug --features e2e-tests', {
			cwd: ROOT,
			stdio: 'inherit',
			env: envWithoutWdioLoader({
				...bakedEnv,
				VITE_E2E: 'true',
				IPHONEOS_DEPLOYMENT_TARGET: '17.0',
				// Drop debuginfo (a full-debug iOS build is >10GB and fills the disk;
				// this is compile-time, so it can't break signing). Unlike android,
				// do NOT also STRIP symbols post-build — that leaves the disk/temp
				// churn that made the .ipa export's codesign fail.
				CARGO_PROFILE_DEV_DEBUG: '0',
			}),
		});

		mkdirSync(path.dirname(SESSION_IPA), { recursive: true });
		// COPYFILE_FICLONE: APFS clones instead of duplicating the ~135MB payload.
		copyFileSync(builtIpa(), SESSION_IPA, constants.COPYFILE_FICLONE);
	}

	async beforeSession() {
		assertBakedHostIpIsStillOurs();
		for (const logger of this.loggers.values()) logger.kill();
		this.loggers.clear();
		// Every spec's `before` creates a profile, so each session needs the app
		// back at first launch — hence a reinstall per spec, not just per run.
		// Doing it here, one device at a time, keeps it off the session's own
		// `appium:app` install, which pushes the same ~135MB .ipa on both devices
		// at once and starves the 5s usbmuxd read timeouts inside appium-ios-device
		// ("Failed to receive any data within the timeout: 5000" out of POST
		// /session). It also stops the app being "installing or uninstalling" when
		// FrontBoard is asked to open it ("Application … is unknown to FrontBoard").
		for (const udid of this.udids.values()) {
			await reinstallApp(udid);
		}
		// Attach the console tails only once the installs are done: idevicesyslog
		// streams the whole device log over the same usbmux channel.
		for (const [slot, udid] of this.udids) {
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
		// The launcher never owned the workers' tails, so kill them by device or
		// the run leaves one attached per spec, blocking the next run's logging.
		for (const udid of this.udids.values()) killStaleSyslogLoggers(udid);
	}
}
