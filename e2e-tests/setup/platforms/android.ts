import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { echoLinesWithPrefix } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { hashFile } from '../device-installs';
import { envWithoutWdioLoader } from '../harness-env';
import { runTurboBuild } from '../turbo-build';
import {
	type AgentPlatform,
	type PrepareContext,
	remoteBakedEnv,
} from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');
const E2E_DIR = path.resolve(__dirname, '..', '..');

const APK_DIR = path.join(ROOT, 'src-tauri/gen/android/app/build/outputs/apk');
export const APP_PACKAGE = 'studio.darksoil.dashchat';

// ABIs the e2e APK build covers (--split-per-abi): the gradle flavor that
// names each APK and the rust target passed to tauri.
const ABIS: Record<string, { flavor: string; target: string }> = {
	'arm64-v8a': { flavor: 'arm64', target: 'aarch64' },
	'armeabi-v7a': { flavor: 'arm', target: 'armv7' },
	x86_64: { flavor: 'x86_64', target: 'x86_64' },
};

// The device's own loopback port baked into the APK as MAILBOX_URL; onPrepare
// bridges it to the host's mailbox via adb reverse.
const DEVICE_MAILBOX_PORT = 3200;

// The device's loopback port baked as PUSH_NOTIFICATIONS_SERVER_URL for the
// real-device push spec; bridged to the host's push server via adb reverse.
const DEVICE_PUSH_PORT = 3201;

/** `android` = physical device, `android-emulator` = running emulator. */
export type AndroidKind = 'android' | 'android-emulator';

/** Appium's extension home — the uiautomator2 driver installs here, not in
 *  node_modules. Without it appium falls back to the pnpm workspace, where
 *  npm-driven driver installs break on pnpm-managed packages. */
const APPIUM_HOME = path.join(E2E_DIR, '.appium');

/** Flake-pinned chromedrivers (one per device WebView major), provided at
 *  this path by the androidDev shellHook; Appium picks the matching one per
 *  device. */
const CHROMEDRIVERS_DIR = path.join(E2E_DIR, '.appium', 'chromedrivers');

// Lives in .appium, not .dbs/e2e: onPrepare wipes .dbs/e2e after the launcher
// (config load) has already written the capture.
const ANDROID_ENV_FILE = path.join(E2E_DIR, '.appium', 'android-env.json');

/** Env the android commands (adb, boot-emulator, the APK build) run with:
 *  process.env when the harness already has the androidDev shell's tools,
 *  otherwise the shell env captured by ensureAndroidEnv. */
let androidEnv: NodeJS.ProcessEnv = process.env;

/** Run an adb shell command on `udid` with the captured android env and
 * return its stdout. */
export function adbShell(udid: string, command: string): string {
	return execSync(`adb -s ${udid} shell ${command}`, {
		encoding: 'utf8',
		env: androidEnv,
	});
}

function androidToolsAvailable(env: NodeJS.ProcessEnv): boolean {
	try {
		execSync('command -v adb', { stdio: 'ignore', env });
	} catch {
		return false;
	}
	return existsSync(CHROMEDRIVERS_DIR);
}

/** One `nix develop` call captures the androidDev shell's full env; its
 *  shellHook also materializes CHROMEDRIVERS_DIR as a side effect. */
function captureAndroidDevShellEnv(): NodeJS.ProcessEnv {
	console.log('Capturing the androidDev shell env (nix develop)...');
	const out = execSync(
		`nix develop "git+file:${ROOT}#androidDev" --command env -0`,
		{ encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
	);
	const env: NodeJS.ProcessEnv = {};
	for (const entry of out.split('\0')) {
		const i = entry.indexOf('=');
		if (i > 0) env[entry.slice(0, i)] = entry.slice(i + 1);
	}
	return env;
}

// The appium server is spawned by @wdio/appium-service with this process's
// env, and its uiautomator2 driver locates adb through PATH/ANDROID_HOME —
// the one part of the captured shell env that must land in process.env.
function applyAppiumServerEnv() {
	for (const key of ['PATH', 'ANDROID_HOME', 'ANDROID_SDK_ROOT']) {
		const value = androidEnv[key];
		if (value !== undefined) process.env[key] = value;
	}
}

// The nix host-build/runtime vars the flake's hostBuildEnvHook exports (see
// flake.nix): the linux RUSTFLAGS bake the tauri libraries' rpath into the
// desktop binary, and the mesa LD_LIBRARY_PATH / LIBGL / EGL vars point its
// WebKitGTK web process at software rendering off NixOS. In a mixed
// android+desktop run the user drives the harness from a plain shell (the
// android capture provides tauri-driver and the rest via PATH), so without
// these the desktop build links against the nix libraries but bakes no rpath —
// the binary then fails at launch with "libpango-1.0.so.0: cannot open shared
// object file". Only fill in vars the outer shell didn't already set, so a run
// started inside the nix dev shell keeps its own values.
function applyHostBuildEnv() {
	const keys = [
		'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS',
		'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS',
		'LD_LIBRARY_PATH',
		'LIBGL_ALWAYS_SOFTWARE',
		'LIBGL_DRIVERS_PATH',
		'__EGL_VENDOR_LIBRARY_DIRS',
	];
	for (const key of keys) {
		const value = androidEnv[key];
		if (value !== undefined && process.env[key] === undefined) {
			process.env[key] = value;
		}
	}
}

/** Make the androidDev shell's tools reachable without wrapping the whole
 *  harness in `nix develop`: capture the shell env once in the launcher and
 *  pass it explicitly to every android command. The capture is pinned to a
 *  file so wdio workers reuse it instead of re-evaluating the flake. */
function ensureAndroidEnv() {
	const pinned = process.env._WDIO_ANDROID_ENV_FILE;
	if (pinned !== undefined) {
		androidEnv = JSON.parse(readFileSync(pinned, 'utf8')) as NodeJS.ProcessEnv;
		applyHostBuildEnv();
		return;
	}
	androidEnv = captureAndroidDevShellEnv();
	mkdirSync(path.dirname(ANDROID_ENV_FILE), { recursive: true });
	writeFileSync(ANDROID_ENV_FILE, JSON.stringify(androidEnv));
	process.env._WDIO_ANDROID_ENV_FILE = ANDROID_ENV_FILE;
	applyAppiumServerEnv();
	applyHostBuildEnv();
	if (!androidToolsAvailable(androidEnv)) {
		throw new Error(
			'adb or the chromedrivers dir still missing after capturing the androidDev shell env',
		);
	}
}

function deviceAbi(udid: string): string {
	const abilist = execSync(
		`adb -s ${udid} shell getprop ro.product.cpu.abilist`,
		{ encoding: 'utf8', env: androidEnv },
	).trim();
	const abi = abilist.split(',').find(a => a in ABIS);
	if (abi === undefined) {
		throw new Error(`No e2e APK flavor for device ${udid} (abis: ${abilist})`);
	}
	return abi;
}

function apkForDevice(udid: string): string {
	const flavor = ABIS[deviceAbi(udid)].flavor;
	return path.join(APK_DIR, flavor, 'debug', `app-${flavor}-debug.apk`);
}

function warnAboutUnauthorizedDevices() {
	const out = execSync('adb devices', { encoding: 'utf8', env: androidEnv });
	if (/unauthorized$/m.test(out)) {
		console.warn(
			'Note: an unauthorized device is connected — accept its USB ' +
				'debugging prompt to use it.',
		);
	}
}

/** Boot a headless emulator for each android-emulator agent that doesn't
 *  have a running emulator yet. Emulators stay running across runs (kill
 *  with `just android kill-emulator`). */
function bootMissingEmulators(needed: number) {
	const running = connectedDevices().filter(d =>
		d.startsWith('emulator-'),
	).length;
	if (running >= needed) return;
	const toBoot = needed - running;

	const logFile = path.join(ROOT, '.dbs', 'e2e', 'emulator.log');
	mkdirSync(path.dirname(logFile), { recursive: true });
	console.log(`Booting ${toBoot} headless emulator(s) (log: ${logFile})...`);
	// Boots run in parallel, but each launch waits for the previous emulator
	// to register with adb: run-test-emulator picks its port by scanning
	// `adb devices`, so unstaggered launches would race for the same port.
	// The boot-wait loops have no timeout — bound them here. Emulator stdio
	// must go to the file: the emulators outlive this call and would hold a
	// pipe open forever.
	const script = `
		set -euo pipefail
		registered() { adb devices | grep -c '^emulator-' || true; }
		base=$(registered)
		pids=()
		for i in $(seq ${toBoot}); do
			boot-emulator < /dev/null >> '${logFile}' 2>&1 &
			pids+=($!)
			until [ "$(registered)" -ge $((base + i)) ]; do sleep 1; done
		done
		for pid in "\${pids[@]}"; do wait "$pid"; done
	`;
	try {
		execSync(script, {
			shell: 'bash',
			timeout: 600_000 * toBoot,
			env: androidEnv,
		});
	} catch (err) {
		const lines = readFileSync(logFile, 'utf8').split('\n');
		console.error(lines.slice(-100).join('\n'));
		throw err;
	}
	console.log('Emulators ready.');
}

/** The uiautomator2 driver lives in APPIUM_HOME (not node_modules); install
 *  it on first run. Pinned to the last version compatible with appium 2.x. */
function ensureUiautomator2Driver() {
	const installed = execSync(
		'pnpm exec appium driver list --installed 2>&1 || true',
		{
			encoding: 'utf8',
			cwd: E2E_DIR,
			env: envWithoutWdioLoader({ APPIUM_HOME }, androidEnv),
		},
	);
	if (!installed.includes('uiautomator2')) {
		execSync('pnpm exec appium driver install uiautomator2@4.2.9', {
			stdio: 'inherit',
			cwd: E2E_DIR,
			env: envWithoutWdioLoader({ APPIUM_HOME }, androidEnv),
		});
	}
}

function connectedDevices(): string[] {
	const out = execSync('adb devices', { encoding: 'utf8', env: androidEnv });
	return out
		.split('\n')
		.slice(1)
		.filter(line => line.trim().endsWith('device'))
		.map(line => line.split('\t')[0]);
}

/**
 * Claim one connected device per slot, matching each slot's kind: physical
 * devices for `android`, `emulator-*` serials for `android-emulator` (the run
 * script boots emulators before wdio starts). `ANDROID_UDID{slot}` overrides
 * the claim for that slot.
 *
 * Like allocatePinnedPort, the claim is pinned via `_WDIO_ANDROID_UDID{slot}`
 * env vars so the launcher and worker config loads agree on the udid->slot
 * mapping even if `adb devices` ordering changes between them.
 */
function claimDevices(
	kindBySlot: Map<number, AndroidKind>,
): Map<number, string> {
	const devices = connectedDevices();
	const pools: Record<AndroidKind, string[]> = {
		android: devices.filter(d => !d.startsWith('emulator-')),
		'android-emulator': devices.filter(d => d.startsWith('emulator-')),
	};
	const udids = new Map<number, string>();
	for (const [slot, kind] of kindBySlot) {
		const pinned =
			process.env[`_WDIO_ANDROID_UDID${slot}`] ??
			process.env[`ANDROID_UDID${slot}`];
		if (pinned !== undefined) {
			udids.set(slot, pinned);
			process.env[`_WDIO_ANDROID_UDID${slot}`] = pinned;
			for (const pool of Object.values(pools)) {
				const i = pool.indexOf(pinned);
				if (i !== -1) pool.splice(i, 1);
			}
			continue;
		}
		const udid = pools[kind].shift();
		if (udid === undefined) {
			throw new Error(
				kind === 'android'
					? `Not enough physical Android devices connected for agent${slot} ` +
						`(connected: ${devices.join(', ') || 'none'}). Connect a device ` +
						`with USB debugging enabled, or set ANDROID_UDID${slot}.`
					: `No running emulator left for agent${slot} ` +
						`(connected: ${devices.join(', ') || 'none'}). Boot one with ` +
						`'just android boot-emulator'.`,
			);
		}
		process.env[`_WDIO_ANDROID_UDID${slot}`] = udid;
		udids.set(slot, udid);
	}
	return udids;
}

/** Tail the app's logcat output on a device and echo it with an agent prefix.
 *  Filters by uid, not pid, so logs survive the app being killed. */
function startLogcatLogger(agent: string, udid: string): ChildProcess {
	const proc = spawn(
		'adb',
		[
			'-s',
			udid,
			'shell',
			`until uid=$(pm list packages -U ${APP_PACKAGE} | sed -n 's/.*uid://p') && ` +
				'[ -n "$uid" ]; do sleep 1; done; logcat -T 1 --uid=$uid',
		],
		{ stdio: ['ignore', 'pipe', 'ignore'], env: androidEnv },
	);
	echoLinesWithPrefix(agent, proc.stdout!);
	return proc;
}

/** Wait until the OS has verified the app's App Links domains. Verification
 *  runs asynchronously after install (the debug key is listed in the hosted
 *  assetlinks.json), and an unforced VIEW intent sent before it completes
 *  would resolve to the browser instead of the app. */
export async function waitForAppLinksVerified(udid: string): Promise<void> {
	const deadline = Date.now() + 30_000;
	for (;;) {
		const out = execSync(
			`adb -s ${udid} shell pm get-app-links ${APP_PACKAGE}`,
			{ encoding: 'utf8', timeout: 10_000, env: androidEnv },
		);
		if (/:\s*verified/.test(out)) return;
		if (Date.now() > deadline) {
			throw new Error(
				`App Links for ${APP_PACKAGE} never became verified:\n${out}`,
			);
		}
		await new Promise(resolve => setTimeout(resolve, 500));
	}
}

/** The md5 of the APK installed on `udid`, or null when not installed. */
function installedApkMd5(udid: string): string | null {
	try {
		const paths = execSync(`adb -s ${udid} shell pm path ${APP_PACKAGE}`, {
			encoding: 'utf8',
			env: androidEnv,
		});
		const base = paths
			.split('\n')
			.map(line => line.trim())
			.find(line => line.startsWith('package:') && line.endsWith('base.apk'));
		if (base === undefined) return null;
		const sum = execSync(
			`adb -s ${udid} shell md5sum "${base.slice('package:'.length)}"`,
			{ encoding: 'utf8', env: androidEnv },
		);
		return sum.trim().split(/\s+/)[0];
	} catch {
		return null;
	}
}

/** Install the e2e APK on `udid` unless it already has this exact build.
 *  Sessions carry no `appium:app`, so this per-run install is the only one —
 *  each session then just fast-resets (`pm clear`) instead of reinstalling. */
/** Comfortably longer than the slowest spec, so no wait can outlive the screen. */
const SCREEN_OFF_TIMEOUT_MS = 30 * 60 * 1000;

/** Stop the display sleeping for the length of the run.
 *
 * WebDriver drives the app through `execute` and injected events, none of which
 * count as user activity, so a spec that waits longer than the device's
 * `screen_off_timeout` puts the screen out — which stops the activity and
 * freezes the webview mid-test. The failure looks like the app misbehaving
 * rather than the screen going off, so it reads as a real bug in whatever was
 * being asserted. `stayon true` covers every plug type rather than just `usb`:
 * a device on the cable can report itself AC-powered, and then a USB-only hold
 * silently never engages. */
function keepScreenAwake(udid: string): void {
	try {
		execSync(`adb -s ${udid} shell svc power stayon true`, { env: androidEnv });
		// `stayon` alone has been observed not to engage even once set, so raise
		// the timeout too rather than trust one of them. Both are persistent
		// device settings, which is the norm for a dedicated test device.
		execSync(
			`adb -s ${udid} shell settings put system screen_off_timeout ${SCREEN_OFF_TIMEOUT_MS}`,
			{ env: androidEnv },
		);
	} catch (err) {
		// Not fatal: it only costs us the flake it prevents.
		console.warn(`[android] could not keep ${udid} awake: ${String(err)}`);
	}
}

function ensureApkInstalled(udid: string): void {
	const apk = apkForDevice(udid);
	if (!existsSync(apk)) {
		throw new Error(
			`e2e APK not found at ${apk} (for device ${udid}) after the tauri android build`,
		);
	}
	if (installedApkMd5(udid) === hashFile(apk, 'md5')) {
		console.log(
			`[android] ${udid} already has the current e2e APK — skipping install`,
		);
		return;
	}
	// Uninstall first: a previous dash-chat (dev build, older e2e APK) fails
	// the install on a signature or version mismatch.
	try {
		execSync(`adb -s ${udid} uninstall ${APP_PACKAGE}`, {
			stdio: 'ignore',
			env: androidEnv,
		});
	} catch {
		/* not installed */
	}
	console.log(`[android] installing the e2e APK on ${udid}...`);
	execSync(`adb -s ${udid} install "${apk}"`, {
		stdio: 'inherit',
		timeout: 300_000,
		env: androidEnv,
	});
}

/** Kill the app on `udid` and wait for its process to die. Unlike
 *  `force-stop`, `stop-app` leaves the package eligible for FCM delivery. */
export function stopAndroidApp(udid: string): void {
	execSync(
		`adb -s ${udid} shell "am stop-app ${APP_PACKAGE} && ` +
			`until ! pidof ${APP_PACKAGE} >/dev/null; do sleep 0.2; done"`,
		{ stdio: 'ignore', timeout: 30_000, env: androidEnv },
	);
}

/**
 * Agents running the e2e APK on Android devices (physical or emulator)
 * through Appium (UiAutomator2) sessions that land directly in the app's
 * webview context, so specs and page objects work exactly as on desktop.
 *
 * The e2e APK is built with MAILBOX_URL=http://127.0.0.1:3200 baked in;
 * onPrepare bridges each device's loopback port 3200 to the host's mailbox
 * server via `adb reverse`.
 */
export class AndroidPlatform implements AgentPlatform {
	readonly slots: number[];
	readonly appiumPort: number;
	private udids: Map<number, string>;
	private loggers = new Map<number, ChildProcess>();

	constructor(kindBySlot: Map<number, AndroidKind>) {
		ensureAndroidEnv();
		this.slots = [...kindBySlot.keys()];
		this.appiumPort = allocatePinnedPort('_WDIO_APPIUM_PORT');
		process.env.APPIUM_HOME = APPIUM_HOME;
		// Workers reload this module; device provisioning belongs to the
		// launcher, which claims before any worker starts.
		if (process.env.WDIO_WORKER_ID === undefined) {
			warnAboutUnauthorizedDevices();
			bootMissingEmulators(
				[...kindBySlot.values()].filter(k => k === 'android-emulator').length,
			);
		}
		this.udids = claimDevices(kindBySlot);
	}

	remoteOptions(slot: number) {
		const udid = this.udids.get(slot)!;
		return {
			port: this.appiumPort,
			capabilities: {
				platformName: 'Android',
				'appium:automationName': 'UiAutomator2',
				'appium:udid': udid,
				// No `appium:app`: onPrepare installs the APK once per run, so a
				// session doesn't push and install it all over again per spec. With
				// only appPackage set, the driver's default fast reset puts each spec
				// back to first-launch state with a ~1s `pm clear` (re-granting
				// permissions per autoGrantPermissions) instead of a reinstall.
				'appium:appPackage': APP_PACKAGE,
				'appium:appActivity': '.MainActivity',
				'appium:autoGrantPermissions': true,
				'appium:autoWebview': true,
				'appium:autoWebviewTimeout': 30_000,
				'appium:systemPort': allocatePinnedPort(`_WDIO_SYSTEM_PORT${slot}`),
				'appium:chromedriverPort': allocatePinnedPort(
					`_WDIO_CHROMEDRIVER_PORT${slot}`,
				),
				'appium:chromedriverExecutableDir': CHROMEDRIVERS_DIR,
				// Costs a devtools round trip per getContexts call, which `startApp`
				// polls after every relaunch. It also refines the chromedriver pick out
				// of CHROMEDRIVERS_DIR, so if a device's WebView major stops matching,
				// restore it first.
				'appium:enableWebviewDetailsCollection': false,
				// terminateApp destroys the webview, so a merely suspended chromedriver
				// session gets handed back stale on the next context switch.
				'appium:recreateChromeDriverSessions': true,
				'appium:adbExecTimeout': 60_000,
				// 0 disables idle expiry: specs like review-checks park one agent
				// for the whole spec after setup, far beyond any sane timeout.
				'appium:newCommandTimeout': 0,
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare(ctx: PrepareContext) {
		ensureUiautomator2Driver();

		// Build the e2e APKs only for the claimed devices' architectures.
		// Debuginfo is dropped and symbols stripped (debug_assertions stays
		// on, which is what makes the webview inspectable) — with them the
		// APK is over 3GB.
		const targets = new Set(
			[...this.udids.values()].map(udid => ABIS[deviceAbi(udid)].target),
		);
		// Against a local mailbox the APK bakes the device's own loopback ports,
		// bridged to the host below; against a remote deployment it bakes that
		// deployment's urls directly and there is nothing to bridge.
		const mailboxEnv: Record<string, string> =
			ctx.mailboxPort === null
				? remoteBakedEnv()
				: {
						MAILBOX_URL: `http://127.0.0.1:${DEVICE_MAILBOX_PORT}`,
						...(ctx.pushPort !== null
							? {
									PUSH_NOTIFICATIONS_SERVER_URL: `http://127.0.0.1:${DEVICE_PUSH_PORT}`,
								}
							: {}),
					};
		const bakedEnv: Record<string, string> = {
			...mailboxEnv,
			CARGO_PROFILE_DEV_DEBUG: '0',
			CARGO_PROFILE_DEV_STRIP: 'symbols',
			E2E_ANDROID_TARGETS: [...targets].join(' '),
		};
		runTurboBuild(
			'e2e:build:android',
			envWithoutWdioLoader(bakedEnv, androidEnv),
		);

		for (const udid of this.udids.values()) {
			ensureApkInstalled(udid);
			keepScreenAwake(udid);

			// A remote mailbox is reached directly; only a local one is bridged.
			if (ctx.mailboxPort === null) continue;
			// Bridge the device's loopback port (baked into the APK) to the
			// host's mailbox server over USB.
			execSync(
				`adb -s ${udid} reverse tcp:${DEVICE_MAILBOX_PORT} tcp:${ctx.mailboxPort}`,
				{ env: androidEnv },
			);
			// Same for the push server, when the real-device push spec is enabled.
			if (ctx.pushPort !== null) {
				execSync(
					`adb -s ${udid} reverse tcp:${DEVICE_PUSH_PORT} tcp:${ctx.pushPort}`,
					{ env: androidEnv },
				);
			}
		}
	}

	async beforeSession() {
		for (const [slot, udid] of this.udids) {
			this.loggers.get(slot)?.kill();
			this.loggers.set(slot, startLogcatLogger(`agent-${slot}`, udid));
		}
	}

	async afterSession() {
		for (const logger of this.loggers.values()) {
			logger.kill();
		}
		this.loggers.clear();
	}

	async onComplete() {
		for (const udid of this.udids.values()) {
			for (const port of [DEVICE_MAILBOX_PORT, DEVICE_PUSH_PORT]) {
				try {
					execSync(`adb -s ${udid} reverse --remove tcp:${port}`, {
						env: androidEnv,
					});
				} catch {
					/* device gone or reverse already removed */
				}
			}
		}
		for (const logger of this.loggers.values()) {
			logger.kill();
		}
	}
}
