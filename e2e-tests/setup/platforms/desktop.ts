import { type ChildProcess, execSync, spawn } from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import {
	killAllE2EProcesses,
	killAndWait,
	killPortHolders,
	pidsNamedWithEnv,
} from '../cleanup';
import { envWithoutWdioLoader } from '../harness-env';
import { E2E_NETWORK_ID } from '../network-id';
import { runTurboBuild } from '../turbo-build';
import { waitForPortFree, waitForPortListening } from '../wait-for-port';
import type { AgentPlatform } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');

/** Desktop agents are this checkout's e2e build of the app, one process per
 *  slot, driven through the W3C WebDriver server that build embeds
 *  (tauri-plugin-wdio-webdriver) on the port TAURI_WEBDRIVER_PORT names.
 *  The harness launches the process itself and the session goes straight to
 *  it: no tauri-driver, and so the same path on Linux and macOS, where no
 *  WebDriver for WKWebView exists. (The compat suite still drives released
 *  builds, which have no server, through tauri-driver.) */
const MACOS = process.platform === 'darwin';

const APP_BINARY = path.join(ROOT, 'target', 'debug', 'dash-chat');

interface DesktopAgent {
	slot: number;
	port: number;
	logger?: ChildProcess | null;
}

function agentDir(slot: number): string {
	return path.join(ROOT, '.dbs', 'e2e', `agent-${slot}`);
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
 *  which nothing here launched and no session can reattach to. */
export function killAgentApp(slot: number) {
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
}

/** The apps this worker launched, by slot. */
const launched = new Map<number, ChildProcess>();

/** What the app runs with on Linux besides the agent's own env: the opener
 *  stub on PATH, and WebKitGTK kept off the paths that hang under a driver. */
function linuxEnv(slot: number): Record<string, string> {
	return {
		PATH: `${installXdgOpenStub(slot)}:${process.env.PATH ?? ''}`,
		// Disable AT-SPI accessibility bridge to prevent D-Bus contention.
		NO_AT_BRIDGE: '1',
		GTK_A11Y: 'none',
		// Disable the DMA-BUF renderer — it causes non-deterministic WebKitGTK
		// freezes. See https://github.com/tauri-apps/tauri/issues/13498
		WEBKIT_DISABLE_DMABUF_RENDERER: '1',
	};
}

/** Launch the agent's app and resolve once its embedded WebDriver server is
 *  listening. */
export async function launchAgentApp(slot: number): Promise<void> {
	const mailboxUrl = process.env.MAILBOX_URL;
	if (mailboxUrl === undefined) {
		throw new Error('MAILBOX_URL not set — onPrepare must run first');
	}
	const port = allocatePinnedPort(`_WDIO_PORT${slot}`);
	const app = spawn(APP_BINARY, [], {
		stdio: 'ignore',
		env: {
			...process.env,
			...(MACOS ? {} : linuxEnv(slot)),
			DATA_DIR: agentDir(slot),
			MAILBOX_URL: mailboxUrl,
			E2E_NETWORK_ID,
			TAURI_WEBDRIVER_PORT: String(port),
		},
	});
	launched.set(slot, app);
	await waitForPortListening(port);
	if (MACOS) raiseMacApp(app.pid);
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

/** Agents running the desktop binary, one app process per slot. */
export class DesktopPlatform implements AgentPlatform {
	private agents: DesktopAgent[];

	constructor(readonly slots: number[]) {
		this.agents = slots.map(slot => ({
			slot,
			port: allocatePinnedPort(`_WDIO_PORT${slot}`),
		}));
	}

	private get ports(): number[] {
		return this.agents.map(a => a.port);
	}

	remoteOptions(slot: number) {
		const agent = this.agents.find(a => a.slot === slot)!;
		return {
			port: agent.port,
			capabilities: {
				platformName: MACOS ? 'mac' : 'linux',
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare() {
		runTurboBuild(
			'e2e:build:desktop',
			envWithoutWdioLoader({
				VITE_E2E: 'true',
				CARGO_PROFILE_DEV_DEBUG: '0',
			}),
		);
		// Kill any leftover processes from previous interrupted runs.
		killAllE2EProcesses();
		killPortHolders(this.ports);
	}

	async beforeSession() {
		// Force-kill any leftover processes from the previous session.
		await this.killLaunched();
		killAllE2EProcesses();
		// Kill anything still holding our specific ports (handles orphaned
		// dash-chat processes that inherited a listening socket).
		killPortHolders(this.ports);
		// Wait for ports to be fully released after SIGKILL.
		await Promise.all(this.ports.map(p => waitForPortFree(p)));

		for (const agent of this.agents) {
			// Clean all agent data for a fresh start (important for
			// specFileRetries). Must remove the entire agent directory, not just
			// the Rust backend data, because WebKitGTK stores
			// localStorage/IndexedDB under the XDG dirs (.local/share/, .config/,
			// .cache/) inside the agent directory.
			const dataDir = agentDir(agent.slot);
			try {
				rmSync(dataDir, { recursive: true, force: true });
			} catch {
				/* ignore */
			}
			mkdirSync(dataDir, { recursive: true });

			// tauri-plugin-log names the file after productName (tauri.conf.json).
			agent.logger = startAgentLogger(
				`agent-${agent.slot}`,
				path.join(dataDir, 'logs', 'Dash Chat.log'),
			);

			await launchAgentApp(agent.slot);
		}
	}

	async afterSession() {
		// SIGKILL the apps launched here and wait for exit to free ports.
		await this.killLaunched();
		// Kill orphaned dash-chat E2E instances and anything holding our ports.
		killAllE2EProcesses();
		killPortHolders(this.ports);
		for (const agent of this.agents) {
			agent.logger?.kill();
			agent.logger = null;
		}
	}

	async onComplete() {
		await this.killLaunched();
		killAllE2EProcesses();
		killPortHolders(this.ports);
		for (const agent of this.agents) {
			agent.logger?.kill();
		}
	}

	private async killLaunched(): Promise<void> {
		await Promise.all(this.agents.map(a => killAndWait(launched.get(a.slot))));
	}
}
