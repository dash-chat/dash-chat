import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { cleanBuildEnv } from '../build-env';
import { killAllE2EProcesses, killAndWait, killPortHolders } from '../cleanup';
import { waitForPortFree, waitForPortListening } from '../wait-for-port';
import type { AgentPlatform } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..', '..');

/** The desktop agent drives the app through `tauri-driver`, which targets
 *  Linux/WebKitGTK (see the GTK/XDG env in beforeSession). tauri-driver has no
 *  macOS backend — it exits with "not supported on this platform" — so a desktop
 *  agent can't run on a Mac; pair iOS agents with each other (PLATFORMS=ios,ios)
 *  or run desktop on Linux. Fail early with the real reason instead of a bare
 *  10s "port not listening" timeout mid-session. */
function assertTauriDriverAvailable() {
	if (process.platform === 'darwin') {
		throw new Error(
			'Desktop e2e agents are not supported on macOS: tauri-driver has no macOS ' +
				'WebView WebDriver backend and the desktop path targets Linux/WebKitGTK. ' +
				'On a Mac run iOS-only (PLATFORMS=ios) or two devices (PLATFORMS=ios,ios); ' +
				'the desktop agent must run on Linux.',
		);
	}
	try {
		execSync('command -v tauri-driver', { stdio: 'ignore' });
	} catch {
		throw new Error(
			'tauri-driver not found — the desktop agent drives the app through it. ' +
				'Run inside the nix dev shell, or install it with `cargo install tauri-driver`.',
		);
	}
}

interface DesktopAgent {
	slot: number;
	port: number;
	nativePort: number;
	driver?: ChildProcess;
	logger?: ChildProcess | null;
}

/** Agents running the desktop binary, one tauri-driver instance per slot. */
export class DesktopPlatform implements AgentPlatform {
	private agents: DesktopAgent[];

	constructor(readonly slots: number[]) {
		assertTauriDriverAvailable();
		this.agents = slots.map(slot => ({
			slot,
			port: allocatePinnedPort(`_WDIO_PORT${slot}`),
			nativePort: allocatePinnedPort(`_WDIO_NATIVE_PORT${slot}`),
		}));
	}

	private get ports(): number[] {
		return this.agents.flatMap(a => [a.port, a.nativePort]);
	}

	remoteOptions(slot: number) {
		const agent = this.agents.find(a => a.slot === slot)!;
		return {
			port: agent.port,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(ROOT, 'target', 'debug', 'dash-chat'),
				},
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare() {
		execSync('pnpm tauri build --debug --no-bundle --features e2e-tests', {
			cwd: ROOT,
			stdio: 'inherit',
			// VITE_E2E reaches the frontend as import.meta.env.VITE_E2E, which
			// compiles development-only chrome out of the binary under test.
			// cleanBuildEnv keeps pnpm working when spawned from the harness in a
			// plain shell (e.g. a mixed ios,desktop run). Drop debuginfo to keep the
			// build small — it isn't needed, and a mixed run builds two targets.
			env: cleanBuildEnv({ VITE_E2E: 'true', CARGO_PROFILE_DEV_DEBUG: '0' }),
		});
		// Kill any leftover processes from previous interrupted runs.
		killAllE2EProcesses();
		killPortHolders(this.ports);
	}

	async beforeSession() {
		// Force-kill any leftover processes from the previous session.
		await Promise.all(this.agents.map(a => killAndWait(a.driver)));
		killAllE2EProcesses();
		// Kill anything still holding our specific ports (handles orphaned
		// dash-chat processes that inherited tauri-driver's listening sockets).
		killPortHolders(this.ports);
		// Wait for ports to be fully released after SIGKILL.
		await Promise.all(this.ports.map(p => waitForPortFree(p)));

		const mailboxUrl = process.env.MAILBOX_URL;
		if (mailboxUrl === undefined) {
			throw new Error('MAILBOX_URL not set — onPrepare must run first');
		}

		for (const agent of this.agents) {
			// Clean all agent data for a fresh start (important for
			// specFileRetries). Must remove the entire agent directory, not just
			// the Rust backend data, because WebKitGTK stores
			// localStorage/IndexedDB under the XDG dirs (.local/share/, .config/,
			// .cache/) inside the agent directory.
			const agentDir = path.join(ROOT, '.dbs', 'e2e', `agent-${agent.slot}`);
			try {
				rmSync(agentDir, { recursive: true, force: true });
			} catch {
				/* ignore */
			}

			mkdirSync(agentDir, { recursive: true });

			// tauri-plugin-log names the file after productName (tauri.conf.json).
			agent.logger = startAgentLogger(
				`agent-${agent.slot}`,
				path.join(agentDir, 'logs', 'Dash Chat.log'),
			);

			agent.driver = spawn(
				'tauri-driver',
				[
					'--port',
					String(agent.port),
					'--native-port',
					String(agent.nativePort),
				],
				{
					stdio: ['ignore', 'ignore', 'pipe'],
					env: {
						...process.env,
						DATA_DIR: agentDir,
						MAILBOX_URL: mailboxUrl,
						// Disable AT-SPI accessibility bridge to prevent D-Bus
						// contention.
						NO_AT_BRIDGE: '1',
						GTK_A11Y: 'none',
						// Disable the DMA-BUF renderer — it causes
						// non-deterministic WebKitGTK freezes. See
						// https://github.com/tauri-apps/tauri/issues/13498
						WEBKIT_DISABLE_DMABUF_RENDERER: '1',
					},
				},
			);
			agent.driver.stderr?.on('data', (data: Buffer) => {
				console.error(`[tauri-driver:${agent.port}] ${data.toString().trim()}`);
			});
		}

		// Wait for tauri-driver instances to accept connections.
		await Promise.all(this.agents.map(a => waitForPortListening(a.port)));
	}

	async afterSession() {
		// SIGKILL tauri-drivers and wait for exit to free ports.
		await Promise.all(this.agents.map(a => killAndWait(a.driver)));
		// Kill orphaned dash-chat E2E instances and anything holding our ports.
		killAllE2EProcesses();
		killPortHolders(this.ports);
		for (const agent of this.agents) {
			agent.logger?.kill();
			agent.logger = null;
		}
	}

	async onComplete() {
		killAllE2EProcesses();
		killPortHolders(this.ports);
		for (const agent of this.agents) {
			agent.logger?.kill();
		}
	}
}
