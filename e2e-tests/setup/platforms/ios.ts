import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { readFileSync, rmSync } from 'node:fs';
import { networkInterfaces, tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { syncXcodeEnv } from '../../../scripts/sync-xcode-env';
import { echoLinesWithPrefix } from '../agent-logger';
import {
	SLOT_PORT_STRIDE,
	allocatePinnedPort,
	allocatePinnedPortFrom,
} from '../allocate-port';
import {
	type Want,
	claimAllWhenFreeSync,
	describeHeld,
	release,
} from '../claims';
import { deviceHasBuild, recordInstalled } from '../device-installs';
import { envWithoutWdioLoader } from '../harness-env';
import { E2E_NETWORK_ID } from '../network-id';
import { E2E_RELAY_URL } from '../relay';
import { runTurboBuild } from '../turbo-build';
import { deviceUdid, switchToWebview, waitForTestUtils } from '../webview';
import {
	type AgentPlatform,
	type PrepareContext,
	remoteBakedEnv,
} from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(__dirname, '..', '..');

export const APP_BUNDLE_ID = 'studio.darksoil.dashchat';
/** What a reachability probe gets before the network counts as having no
 *  upstream. A captive portal answers fast or not at all. */
const INTERNET_PROBE_TIMEOUT = 5_000;

const APPIUM_BIN = path.join(E2E_DIR, 'node_modules', '.bin', 'appium');
// Fixed home for the .ipa the sessions install, copied here by the
// e2e:build:ios task's export-session-ipa.ts step. The capabilities
// `appium:app` reads are resolved when wdio.conf.ts loads — before onPrepare
// builds anything — so it can't name a path under the build dir,
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
	const pinnedOf = (slot: number) =>
		process.env[`_WDIO_IOS_UDID${slot}`] ?? process.env[`IOS_UDID${slot}`];
	const pinned = new Set(slots.map(pinnedOf).filter(u => u !== undefined));
	const connected = connectedDevices();
	// Slots wanting the same thing — a pinned device, or any iPhone — claim
	// it as one group, so a run holds all it needs or nothing.
	const groups = new Map<string, { slots: number[]; want: Want }>();
	for (const slot of slots) {
		const key = pinnedOf(slot) ?? 'any';
		const group = groups.get(key) ?? {
			slots: [],
			want: {
				candidates:
					key === 'any' ? connected.filter(d => !pinned.has(d)) : [key],
				needed: 0,
			},
		};
		group.slots.push(slot);
		group.want.needed += 1;
		groups.set(key, group);
	}
	for (const { slots: wanting, want } of groups.values()) {
		if (want.candidates.length >= want.needed) continue;
		const slot = wanting[want.candidates.length];
		throw new Error(
			`Not enough iPhones for agent${slot} ` +
				`(${describeHeld(connected)}). Connect a ` +
				`trusted device, or set IOS_UDID${slot}.`,
		);
	}
	const wants = [...groups.values()].map(g => g.want);
	// The launcher claims, waiting for devices another run is driving; the
	// workers inherit its claims through the pinned env.
	const taken =
		process.env.WDIO_WORKER_ID === undefined
			? claimAllWhenFreeSync(wants)
			: wants.map(w => w.candidates.slice(0, w.needed));
	const udids = new Map<number, string>();
	[...groups.values()].forEach(({ slots: wanting }, i) => {
		wanting.forEach((slot, j) => {
			process.env[`_WDIO_IOS_UDID${slot}`] = taken[i][j];
			udids.set(slot, taken[i][j]);
		});
	});
	return udids;
}

/** Appium keeps its drivers in APPIUM_HOME, not node_modules, so on first run
 *  link in the `appium-xcuitest-driver` devDependency: the version is pinned in
 *  package.json (bump it there, then `pnpm install`) and the link tracks it. The
 *  pin must be a version whose bundled WebDriverAgent compiles on current Xcode,
 *  which needs the appium 3.x server (see the `appium` dep); xcuitest <= 9.x
 *  ships a WDA that fails to build ("xcodebuild failed with code 65"). */
function ensureXcuitestDriver() {
	const installed = execSync(
		`"${APPIUM_BIN}" driver list --installed 2>&1 || true`,
		{ encoding: 'utf8', cwd: E2E_DIR },
	);
	if (installed.includes('xcuitest')) return;
	const driverPath = path.join(
		E2E_DIR,
		'node_modules',
		'appium-xcuitest-driver',
	);
	execSync(`"${APPIUM_BIN}" driver install --source=local "${driverPath}"`, {
		stdio: 'inherit',
		cwd: E2E_DIR,
	});
}

/** `mobile: queryAppState` value for "the app is not running". */
export const APP_STATE_NOT_RUNNING = 1;
/** `mobile: queryAppState` value for "the app is on screen". */
export const APP_STATE_FOREGROUND = 4;

/** SIGKILL the phone's push extension process, so the next push starts a
 *  fresh one — with a node built from the app's current data — instead of
 *  waking an old process still holding a node from before. */
export function killIosPushExtension(udid: string): void {
	const listing = path.join(
		tmpdir(),
		`dashchat-processes-${udid}-${process.pid}.json`,
	);
	execSync(
		`xcrun devicectl device info processes --device ${udid} --json-output "${listing}"`,
		{ stdio: 'ignore' },
	);
	const { result } = JSON.parse(readFileSync(listing, 'utf8')) as {
		result: {
			runningProcesses: { executable?: string; processIdentifier: number }[];
		};
	};
	rmSync(listing, { force: true });
	for (const p of result.runningProcesses) {
		if (p.executable?.endsWith('/PushNotificationsExtension') !== true)
			continue;
		execSync(
			`xcrun devicectl device process signal --device ${udid} --pid ${p.processIdentifier} --signal SIGKILL`,
			{ stdio: 'ignore' },
		);
	}
}

/** Reset an iOS agent to first-launch state without reinstalling the app.
 *
 *  iOS has no adb-style data clear, and reinstalling the ~135MB .ipa per spec
 *  is what made device runs slow — so the reset is the app's own
 *  delete_account (the Settings → Account → Delete account code path), which
 *  wipes the data dir and exits the process. Relaunch, and the spec starts
 *  from the same state a fresh install would. */
export async function resetIosAppState(b: WebdriverIO.Browser): Promise<void> {
	await wipeIosAppData(b);
	await attachToIosApp(b);
}

/** Leave the app installed with no data and not running: the iOS answer to
 *  android's `mobile: clearApp`. The wipe is the app's own delete_account, so
 *  the app is brought up to run it and exits on its own afterwards.
 *
 *  Also what a spec file is left in once it is done. The next spec's reset
 *  runs only after the app is up with the old data, and in a new run that app
 *  uploads its whole history to the fresh mailbox, which pushes every message
 *  back to this phone: banners over the navbar the spec is about to tap. */
export async function clearIosAppData(b: WebdriverIO.Browser): Promise<void> {
	await attachToIosApp(b);
	await wipeIosAppData(b);
}

/** Whether the phone can reach the internet, asked of the app's own webview:
 *  iOS has no adb-style shell to run a probe in, and the answer has to come
 *  from the phone, not from the host, which is on a different network.
 *
 *  The probe reads a cross-origin response rather than just seeing a request
 *  leave, so a captive portal answering in the internet's place cannot pass
 *  for it: a portal serves its own page from its own origin, which the browser
 *  refuses to hand back without the CORS header the real endpoint sends.
 *
 *  Brings the app to the foreground to ask, and leaves it there — every caller
 *  wipes and relaunches it next, so what it comes up on does not matter. The
 *  raised script timeout is left raised too: XCUITest's default is ~0, which
 *  is no state worth restoring. */
export async function iosHasInternet(b: WebdriverIO.Browser): Promise<boolean> {
	await attachToIosApp(b);
	// XCUITest defaults the async-script timeout to ~0 (see `agent.disableP2p`).
	await b.setTimeout({ script: INTERNET_PROBE_TIMEOUT + 10_000 });
	return b.executeAsync((ms: number, done: (reachable: boolean) => void) => {
		const timer = setTimeout(() => done(false), ms);
		const settle = (reachable: boolean) => {
			clearTimeout(timer);
			done(reachable);
		};
		// A DNS-over-HTTPS query: small, meant to be read programmatically, and
		// served with `access-control-allow-origin: *`.
		fetch('https://cloudflare-dns.com/dns-query?name=example.com&type=A', {
			headers: { accept: 'application/dns-json' },
			cache: 'no-store',
		}).then(
			response => settle(response.ok),
			() => settle(false),
		);
	}, INTERNET_PROBE_TIMEOUT);
}

/** Run the app's own delete_account, which wipes the data dir and exits, and
 *  end the push extension, whose node would otherwise outlive the data it was
 *  built from and serve the next account's pushes from the old one. */
async function wipeIosAppData(b: WebdriverIO.Browser): Promise<void> {
	await b.execute(() => window.__test.resetToFirstLaunch());
	// The command exits the app; leave the webview before it dies under us.
	await b.switchContext('NATIVE_APP');
	await b.waitUntil(
		async () =>
			Number(
				await b.execute('mobile: queryAppState', { bundleId: APP_BUNDLE_ID }),
			) <= APP_STATE_NOT_RUNNING,
		{ timeoutMsg: 'the app never exited after delete_account' },
	);
	killIosPushExtension(deviceUdid(b));
}

/** Bring the app to the foreground and attach to its webview. */
async function attachToIosApp(b: WebdriverIO.Browser): Promise<void> {
	await b.activateApp(APP_BUNDLE_ID);
	await switchToWebview(b, 'ios');
	await waitForTestUtils(b);
}

/** Where the device says our app's bundle sits right now. Each install puts it
 *  in a container of its own, so this changes whenever anything replaces the
 *  build we installed — a release or TestFlight build carries the same version
 *  and bundle id as ours, and pointed at the cloud mailbox instead of the
 *  run's, it fails every cross-device wait in the suite. `undefined` when the
 *  app is absent or the device can't be asked, which reinstalls. */
function installedBundlePath(udid: string): string | undefined {
	const listing = path.join(
		tmpdir(),
		`dashchat-apps-${udid}-${process.pid}.json`,
	);
	try {
		execSync(
			`xcrun devicectl device info apps --device ${udid} ` +
				`--bundle-id ${APP_BUNDLE_ID} --json-output "${listing}"`,
			{ stdio: 'ignore' },
		);
		const { result } = JSON.parse(readFileSync(listing, 'utf8')) as {
			result: { apps: { url?: string }[] };
		};
		return result.apps[0]?.url;
	} catch {
		return undefined;
	} finally {
		rmSync(listing, { force: true });
	}
}

/** Put a device back to a freshly-installed app, retrying the install: CoreDevice
 * intermittently fails with a transient "unable to create bookmark data" /
 * "No such file" error even though the .ipa exists. Best-effort — a device left
 * without the app falls back to the session's own `appium:app` install.
 * Returns whether the devicectl install actually succeeded. */
async function reinstallApp(udid: string): Promise<boolean> {
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
			return true;
		} catch (err) {
			if (attempt === 3) {
				console.warn(
					`[ios] devicectl install failed for ${udid} after ${attempt} ` +
						`attempts; falling back to the session's appium:app install: ${err}`,
				);
				return false;
			}
			await new Promise(resolve => setTimeout(resolve, 2000));
		}
	}
	return false;
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
				// onPrepare already installed this exact build through devicectl;
				// without this the driver installs it again every session
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
				'appium:wdaLocalPort': allocatePinnedPortFrom(
					`_WDIO_WDA_PORT${slot}`,
					8100 + slot * SLOT_PORT_STRIDE,
				),
				'appium:mjpegServerPort': allocatePinnedPortFrom(
					`_WDIO_MJPEG_PORT${slot}`,
					9100 + slot * SLOT_PORT_STRIDE,
				),
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

	/** The host's LAN address for the local mailbox and push server. */
	private localBakedEnv(
		mailboxPort: number,
		pushPort: number | null,
	): Record<string, string> {
		const hostIp = detectHostIp();
		// Workers inherit the launcher's env, so this is what they check the
		// host still holds before each session.
		process.env._WDIO_IOS_HOST_IP = hostIp;
		const bakedEnv: Record<string, string> = {
			E2E_NETWORK_ID,
			E2E_RELAY_URL,
			MAILBOX_URL: `http://${hostIp}:${mailboxPort}`,
		};
		if (pushPort !== null) {
			bakedEnv.PUSH_NOTIFICATIONS_SERVER_URL = `http://${hostIp}:${pushPort}`;
		}
		return bakedEnv;
	}

	async onPrepare(ctx: PrepareContext) {
		ensureXcuitestDriver();

		// The app reaches its mailbox and push server at whatever this bakes in.
		// Against a local mailbox that is the host's LAN address; against a
		// remote deployment it is that deployment's own mailbox and push server —
		// they must match, since the mailbox notifies its own push server and the
		// device registers its token with the one it was built for.
		const bakedEnv =
			ctx.mailboxPort === null
				? remoteBakedEnv()
				: this.localBakedEnv(ctx.mailboxPort, ctx.pushPort);
		syncXcodeEnv(bakedEnv);
		// The task's last step (scripts/export-session-ipa.ts) copies the built
		// .ipa to SESSION_IPA, so turbo snapshots and restores the final artifact.
		runTurboBuild(
			'e2e:build:ios',
			envWithoutWdioLoader({
				...bakedEnv,
				VITE_E2E: 'true',
				IPHONEOS_DEPLOYMENT_TARGET: '17.0',
				// Drop debuginfo (a full-debug iOS build is >10GB and fills the disk;
				// this is compile-time, so it can't break signing). Unlike android,
				// do NOT also STRIP symbols post-build — that leaves the disk/temp
				// churn that made the .ipa export's codesign fail.
				CARGO_PROFILE_DEV_DEBUG: '0',
			}),
		);

		// Install once per run (not per spec — specs reset state through the
		// app's own delete_account instead), and only on devices that don't
		// already hold this exact build from a previous run.
		for (const udid of this.udids.values()) {
			if (deviceHasBuild(udid, SESSION_IPA, installedBundlePath(udid))) {
				console.log(
					`[ios] ${udid} already has the current e2e build — skipping install`,
				);
				continue;
			}
			if (await reinstallApp(udid)) {
				recordInstalled(udid, SESSION_IPA, installedBundlePath(udid));
			}
		}
	}

	async beforeSession() {
		assertBakedHostIpIsStillOurs();
		for (const logger of this.loggers.values()) logger.kill();
		this.loggers.clear();
		// No reinstall here: onPrepare installed the build once for the whole
		// run, and each spec resets to first-launch state through the app's own
		// delete_account (see resetIosAppState above) — far faster than pushing
		// the ~135MB .ipa over usbmux for every spec.
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
		for (const udid of this.udids.values()) {
			killStaleSyslogLoggers(udid);
			release(udid);
		}
	}
}
