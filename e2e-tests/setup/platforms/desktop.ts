import { type ChildProcess, spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from '../agent-logger';
import { allocatePinnedPort } from '../allocate-port';
import { killAllE2EProcesses, killAndWait, killPortHolders } from '../cleanup';
import { waitForPortFree, waitForPortListening } from '../wait-for-port';
import type { AgentPlatform } from './platform';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SETUP_DIR = path.resolve(__dirname, '..');
const ROOT = path.resolve(__dirname, '..', '..', '..');

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
				platformName:
					process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(SETUP_DIR, `launch-agent${slot}.sh`),
				},
			} as WebdriverIO.Capabilities,
		};
	}

	async onPrepare() {
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

			agent.logger = startAgentLogger(
				`agent-${agent.slot}`,
				path.join(agentDir, 'agent.log'),
			);

			agent.driver = spawn(
				'tauri-driver',
				[
					'--port',
					String(agent.port),
					'--native-port',
					String(agent.nativePort),
				],
				{ stdio: ['ignore', 'ignore', 'pipe'] },
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

	onComplete() {
		killAllE2EProcesses();
		killPortHolders(this.ports);
		for (const agent of this.agents) {
			agent.logger?.kill();
		}
	}
}
