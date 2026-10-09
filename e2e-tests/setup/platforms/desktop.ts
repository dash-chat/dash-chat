import {
	type ChildProcess,
	type StdioOptions,
	execSync,
	spawn,
} from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { remote } from 'webdriverio';

import { UI_TIMEOUT } from '../../helpers/timeouts';
import { startAgentLogger } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { runAppBuild } from '../app-build';
import { killAllE2EProcesses, killAndWait, pidsNamedWithEnv } from '../cleanup';
import { envWithoutWdioLoader } from '../harness-env';
import { E2E_NETWORK_ID } from '../network-id';
import { E2E_RELAY_URL } from '../relay';
import {
	isPortListening,
	waitForPortFree,
	waitForPortListening,
} from '../wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');

/** Desktop agents are this checkout's e2e build of the app, one process per
 *  slot, driven through the W3C WebDriver server that build embeds
 *  (tauri-plugin-wdio-webdriver) on the port TAURI_WEBDRIVER_PORT names.
 *  The harness launches the process itself and the session goes straight to
 *  it: no tauri-driver, and so the same path on Linux and macOS, where no
 *  WebDriver for WKWebView exists. */
const MACOS = process.platform === 'darwin';

export const APP_BINARY = path.join(ROOT, 'target', 'debug', 'dash-chat');

function agentDir(slot: number): string {
	return path.join(ROOT, '.dbs', 'e2e', `agent-${slot}`);
}

function agentPort(slot: number): number {
	return allocatePinnedPort(`_WDIO_PORT${slot}`);
}

function openedUrlsPath(slot: number): string {
	return path.join(agentDir(slot), 'opened-urls');
}

/**
 * Put an `xdg-open` stub at the front of the agent's PATH. `open`, the crate
 * behind tauri-plugin-opener, launches urls with `Command::new("xdg-open")`, so
 * the stub records what the app asked the OS to open — exercising the whole
 * real stack without a browser window appearing mid-run. Linux only: on macOS
 * the crate runs `/usr/bin/open` by its absolute path.
 */
function installXdgOpenStub(slot: number): string {
	const binDir = path.join(agentDir(slot), 'bin');
	mkdirSync(binDir, { recursive: true });
	writeFileSync(
		path.join(binDir, 'xdg-open'),
		`#!/bin/sh\nprintf '%s\\n' "$1" >> ${JSON.stringify(openedUrlsPath(slot))}\n`,
		{ mode: 0o755 },
	);
	return binDir;
}

/** SIGKILL any app process running against this agent's data dir — e.g. the
 *  instance `delete_account` self-restarts into (`tauri::process::restart`),
 *  which nothing here launched and no session can reattach to — and wait until
 *  the agent's WebDriver port is free. */
export async function killAgentApp(slot: number): Promise<void> {
	for (const pid of pidsNamedWithEnv(
		'dash-chat',
		`DATA_DIR=${agentDir(slot)}`,
	)) {
		try {
			process.kill(pid, 'SIGKILL');
		} catch {
			/* already gone */
		}
	}
	await waitForPortFree(agentPort(slot));
}

/** Whether an app is serving WebDriver on this agent's port. */
export async function isAgentAppRunning(slot: number): Promise<boolean> {
	return await isPortListening(agentPort(slot));
}

/** The apps this worker launched, by slot. */
const launched = new Map<number, ChildProcess>();
/** The echo of each launched agent's log, by slot. */
const loggers = new Map<number, ChildProcess>();

/** WebKitGTK kept off the paths that hang under a driver. */
const WEBKITGTK_ENV: Record<string, string> = {
	// Disable AT-SPI accessibility bridge to prevent D-Bus contention.
	NO_AT_BRIDGE: '1',
	GTK_A11Y: 'none',
	// Disable the DMA-BUF renderer — it causes non-deterministic WebKitGTK
	// freezes. See https://github.com/tauri-apps/tauri/issues/13498
	WEBKIT_DISABLE_DMABUF_RENDERER: '1',
};

export function buildDesktopApp(): void {
	runAppBuild(
		'e2e:build:desktop',
		envWithoutWdioLoader({
			VITE_E2E: 'true',
			CARGO_PROFILE_DEV_DEBUG: '0',
			E2E_NETWORK_ID,
			E2E_RELAY_URL,
		}),
	);
}

/** Launch the e2e build at `binary` against `dataDir`, with the harness's
 *  environment and `env` on top, and resolve once the WebDriver server it
 *  embeds is listening on `port`. */
export async function launchDesktopApp(
	binary: string,
	dataDir: string,
	port: number,
	env: NodeJS.ProcessEnv,
	stdio: StdioOptions = 'ignore',
): Promise<ChildProcess> {
	const mailboxUrl = process.env.MAILBOX_URL;
	if (mailboxUrl === undefined) {
		throw new Error('MAILBOX_URL not set — onPrepare must run first');
	}
	const app = spawn(binary, [], {
		stdio,
		env: {
			...process.env,
			...(MACOS ? {} : WEBKITGTK_ENV),
			DATA_DIR: dataDir,
			MAILBOX_URL: mailboxUrl,
			TAURI_WEBDRIVER_PORT: String(port),
			...env,
		},
	});
	try {
		await waitForPortListening(port);
	} catch (e) {
		await killAndWait(app);
		throw e;
	}
	if (MACOS) raiseMacApp(app.pid);
	return app;
}

/** Launch the agent's app and resolve once its embedded WebDriver server is
 *  listening. On Linux the opener stub goes on its PATH. */
export async function launchAgentApp(slot: number): Promise<void> {
	const port = agentPort(slot);
	const app = await launchDesktopApp(
		APP_BINARY,
		agentDir(slot),
		port,
		MACOS
			? {}
			: { PATH: `${installXdgOpenStub(slot)}:${process.env.PATH ?? ''}` },
	);
	launched.set(slot, app);
}

/** Bring the app's windows above every other app's. WebKit stops animation
 *  frames and transitions in a document whose window is covered, and a
 *  popover that fades in never becomes visible there. */
function raiseMacApp(pid: number | undefined): void {
	if (pid === undefined) return;
	try {
		execSync(
			`osascript -e 'tell application "System Events" to set frontmost of (first process whose unix id is ${pid}) to true'`,
			{ stdio: 'ignore' },
		);
	} catch {
		/* no window server session to raise in; the run may still pass */
	}
}

/** Wipe a stopped agent back to first launch. The whole agent directory
 *  goes, not just the Rust backend's data: WebKitGTK keeps localStorage and
 *  IndexedDB under the XDG dirs inside it, so leaving those makes an app that
 *  starts with no account still remember what the webview stored. */
export function clearAgentDir(slot: number): void {
	const dataDir = agentDir(slot);
	// A just-killed agent's WebKit helpers can still be writing into it.
	rmSync(dataDir, {
		recursive: true,
		force: true,
		maxRetries: 10,
		retryDelay: 100,
	});
	mkdirSync(dataDir, { recursive: true });
}

/** Where the macOS window of `slot` goes: agents side by side, so neither
 *  covers the other (see [`raiseMacApp`]). */
export function macWindowRect(slot: number): {
	x: number;
	y: number;
	width: number;
	height: number;
} {
	return { x: 20 + (slot - 1) * 940, y: 60, width: 900, height: 750 };
}

/** The urls this agent asked the OS to open, oldest first. */
export function readOpenedUrls(slot: number): string[] {
	const file = openedUrlsPath(slot);
	if (!existsSync(file)) return [];
	return readFileSync(file, 'utf8')
		.split('\n')
		.filter(line => line !== '');
}

/** Launch a desktop agent on `slot` from first launch — a fresh data dir,
 *  its log echoed — and open a WebDriver session to it. */
export async function launchDesktopAgent(
	slot: number,
): Promise<WebdriverIO.Browser> {
	clearAgentDir(slot);
	// tauri-plugin-log names the file after productName (tauri.conf.json).
	loggers.set(
		slot,
		startAgentLogger(
			`agent-${slot}`,
			path.join(agentDir(slot), 'logs', 'Dash Chat.log'),
		),
	);
	await launchAgentApp(slot);
	return await remote({
		hostname: '127.0.0.1',
		port: agentPort(slot),
		capabilities: {
			platformName: MACOS ? 'mac' : 'linux',
		} as WebdriverIO.Capabilities,
		logLevel: 'warn',
		waitforTimeout: UI_TIMEOUT,
	});
}

/** Kill one desktop agent's app and log echo, and delete what it stored. */
export async function discardDesktopAgent(slot: number): Promise<void> {
	await killAgentApp(slot);
	launched.delete(slot);
	loggers.get(slot)?.kill();
	loggers.delete(slot);
	rmSync(agentDir(slot), { recursive: true, force: true });
}

/** Kill every desktop agent this worker launched, and any app process still
 *  running on this checkout's data dirs, such as one an app restarted itself
 *  into. */
export async function stopDesktopAgents(): Promise<void> {
	await Promise.all([...launched.values()].map(app => killAndWait(app)));
	launched.clear();
	killAllE2EProcesses();
	for (const logger of loggers.values()) logger.kill();
	loggers.clear();
}
