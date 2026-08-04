import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { echoLinesWithPrefix } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import type { AgentPlatform, PrepareContext } from './platform';

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

/** `android` = physical device, `android-emulator` = running emulator. */
export type AndroidKind = 'android' | 'android-emulator';

/** Flake-pinned chromedrivers (one per device WebView major), provided at
 *  this path by the androidDev shellHook; Appium picks the matching one per
 *  device. */
const CHROMEDRIVERS_DIR = path.join(E2E_DIR, '.appium', 'chromedrivers');

function assertAndroidToolsAvailable() {
	try {
		execSync('command -v adb', { stdio: 'ignore' });
	} catch {
		throw new Error(
			'adb not found — Android agents run inside the androidDev dev ' +
				"shell ('just test e2e' handles this)",
		);
	}
	if (!existsSync(CHROMEDRIVERS_DIR)) {
		throw new Error(
			`${CHROMEDRIVERS_DIR} not found — the androidDev shellHook ` +
				"provides it ('just test e2e' handles this)",
		);
	}
}

function deviceAbi(udid: string): string {
	const abilist = execSync(
		`adb -s ${udid} shell getprop ro.product.cpu.abilist`,
		{ encoding: 'utf8' },
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
	const out = execSync('adb devices', { encoding: 'utf8' });
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
		execSync(script, { shell: 'bash', timeout: 600_000 * toBoot });
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
		{ encoding: 'utf8', cwd: E2E_DIR },
	);
	if (!installed.includes('uiautomator2')) {
		execSync('pnpm exec appium driver install uiautomator2@4.2.9', {
			stdio: 'inherit',
			cwd: E2E_DIR,
		});
	}
}

function connectedDevices(): string[] {
	const out = execSync('adb devices', { encoding: 'utf8' });
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

/**
 * Tail the app's logcat output on a device and echo it with an agent prefix.
 * Waits on-device for the app process to exist, then follows its pid.
 */
function startLogcatLogger(agent: string, udid: string): ChildProcess {
	const proc = spawn(
		'adb',
		[
			'-s',
			udid,
			'shell',
			`until pid=$(pidof -s ${APP_PACKAGE}); do sleep 1; done; logcat -T 1 --pid=$pid`,
		],
		{ stdio: ['ignore', 'pipe', 'ignore'] },
	);
	echoLinesWithPrefix(agent, proc.stdout!);
	return proc;
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
		assertAndroidToolsAvailable();
		this.slots = [...kindBySlot.keys()];
		this.appiumPort = allocatePinnedPort('_WDIO_APPIUM_PORT');
		process.env.APPIUM_HOME = path.join(E2E_DIR, '.appium');
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
				'appium:app': apkForDevice(udid),
				'appium:autoGrantPermissions': true,
				'appium:autoWebview': true,
				'appium:autoWebviewTimeout': 30_000,
				'appium:systemPort': allocatePinnedPort(`_WDIO_SYSTEM_PORT${slot}`),
				'appium:chromedriverPort': allocatePinnedPort(
					`_WDIO_CHROMEDRIVER_PORT${slot}`,
				),
				'appium:chromedriverExecutableDir': CHROMEDRIVERS_DIR,
				'appium:adbExecTimeout': 60_000,
				'appium:newCommandTimeout': 240,
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare(ctx: PrepareContext) {
		if (ctx.mailboxPort === null) {
			throw new Error(
				'Android agents need a local mailbox server (the e2e APK bakes http://127.0.0.1:3200)',
			);
		}

		ensureUiautomator2Driver();

		// Build the e2e APKs only for the claimed devices' architectures.
		// Debuginfo is dropped and symbols stripped (debug_assertions stays
		// on, which is what makes the webview inspectable) — with them the
		// APK is over 3GB.
		const targets = new Set(
			[...this.udids.values()].map(udid => ABIS[deviceAbi(udid)].target),
		);
		execSync(
			'pnpm tauri android build --debug --apk --split-per-abi ' +
				`--features e2e-tests -t ${[...targets].join(' ')}`,
			{
				cwd: ROOT,
				stdio: 'inherit',
				env: {
					...process.env,
					MAILBOX_URL: `http://127.0.0.1:${DEVICE_MAILBOX_PORT}`,
					CARGO_PROFILE_DEV_DEBUG: '0',
					CARGO_PROFILE_DEV_STRIP: 'symbols',
				},
			},
		);

		for (const udid of this.udids.values()) {
			const apk = apkForDevice(udid);
			if (!existsSync(apk)) {
				throw new Error(
					`e2e APK not found at ${apk} (for device ${udid}) after the tauri android build`,
				);
			}
		}

		for (const udid of this.udids.values()) {
			// Uninstall any previous dash-chat (dev build, older e2e APK) so the
			// session installs the fresh APK instead of failing on a signature or
			// version mismatch.
			try {
				execSync(`adb -s ${udid} uninstall ${APP_PACKAGE}`, {
					stdio: 'ignore',
				});
			} catch {
				/* not installed */
			}

			// Bridge the device's loopback port (baked into the APK) to the
			// host's mailbox server over USB.
			execSync(
				`adb -s ${udid} reverse tcp:${DEVICE_MAILBOX_PORT} tcp:${ctx.mailboxPort}`,
			);
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
			try {
				execSync(`adb -s ${udid} reverse --remove tcp:${DEVICE_MAILBOX_PORT}`);
			} catch {
				/* device gone or reverse already removed */
			}
		}
		for (const logger of this.loggers.values()) {
			logger.kill();
		}
	}
}
