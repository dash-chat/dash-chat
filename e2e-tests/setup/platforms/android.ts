import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { echoLinesWithPrefix } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import type { AgentPlatform, PrepareContext } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');

const APK_DIR = path.join(ROOT, 'src-tauri/gen/android/app/build/outputs/apk');
const APP_PACKAGE = 'studio.darksoil.dashchat';

// ABIs covered by `just test e2e android-build` (--split-per-abi), mapped to
// the gradle flavor that names each APK.
const ABI_FLAVORS: Record<string, string> = {
	'arm64-v8a': 'arm64',
	'armeabi-v7a': 'arm',
	x86_64: 'x86_64',
};

// Must match the MAILBOX_URL baked into the APK by `just test e2e android-build`.
const DEVICE_MAILBOX_PORT = 3200;

/** `android` = physical device, `android-emulator` = running emulator. */
export type AndroidKind = 'android' | 'android-emulator';

/** Directory of flake-pinned chromedrivers (one per device WebView major);
 *  Appium picks the matching one per device. Exported by the androidDev
 *  dev shell. */
function chromedriversDir(): string {
	const dir = process.env.E2E_CHROMEDRIVERS_DIR;
	if (dir === undefined) {
		throw new Error(
			'E2E_CHROMEDRIVERS_DIR is not set — Android agents must run inside ' +
				"the androidDev dev shell ('just test e2e' handles this)",
		);
	}
	return dir;
}

function apkForDevice(udid: string): string {
	const abilist = execSync(
		`adb -s ${udid} shell getprop ro.product.cpu.abilist`,
		{ encoding: 'utf8' },
	).trim();
	for (const abi of abilist.split(',')) {
		const flavor = ABI_FLAVORS[abi];
		if (flavor !== undefined) {
			return path.join(APK_DIR, flavor, 'debug', `app-${flavor}-debug.apk`);
		}
	}
	throw new Error(`No e2e APK flavor for device ${udid} (abis: ${abilist})`);
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
		this.slots = [...kindBySlot.keys()];
		this.appiumPort = allocatePinnedPort('_WDIO_APPIUM_PORT');
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
				'appium:chromedriverExecutableDir': chromedriversDir(),
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

		for (const udid of this.udids.values()) {
			const apk = apkForDevice(udid);
			if (!existsSync(apk)) {
				throw new Error(
					`e2e APK not found at ${apk} (for device ${udid}). ` +
						`Build it with 'just test e2e android-build'.`,
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
