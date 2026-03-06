import { type ChildProcess, spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';
import { allocateDriverPorts } from '../helpers/allocate-port';
import {
	killAndWait,
	killAllE2EProcesses,
	killPortHolders,
} from '../helpers/cleanup';
import { waitForPortFree, waitForPortListening } from '../helpers/wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(__dirname, '..');
const ROOT = path.resolve(__dirname, '../..');

const phase = process.env.COMPAT_PHASE;
if (!phase || !['setup', 'verify'].includes(phase)) {
	throw new Error('COMPAT_PHASE must be "setup" or "verify"');
}

if (!process.env.COMPAT_BINARY) {
	throw new Error('COMPAT_BINARY env var required');
}

const specFile =
	phase === 'setup'
		? path.join(E2E_DIR, 'specs', 'compat-setup.spec.ts')
		: path.join(E2E_DIR, 'specs', 'compat-verify.spec.ts');

const { port1, nativePort1, port2, nativePort2 } = allocateDriverPorts();
const ALL_PORTS = [port1, nativePort1, port2, nativePort2];

let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;

export const config: Options.Testrunner = {
	runner: 'local',
	tsNodeOpts: { esm: true, project: path.join(E2E_DIR, 'tsconfig.json') },

	specs: [specFile],
	maxInstances: 1,
	specFileRetries: 1,

	capabilities: {
		agent1: {
			port: port1,
			capabilities: {
				'platformName': process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'scripts', 'launch-agent1.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: port2,
			capabilities: {
				'platformName': process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'scripts', 'launch-agent2.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
	},

	logLevel: 'warn',
	waitforTimeout: 30_000,

	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 120_000,
	},

	reporters: ['spec'],

	// No onPrepare build step — the orchestrator handles building

	async beforeSession() {
		// Force-kill any leftover processes from a previous phase.
		await Promise.all([killAndWait(tauriDriver1), killAndWait(tauriDriver2)]);
		killAllE2EProcesses();
		killPortHolders(ALL_PORTS);
		// Wait for ports to be fully released after SIGKILL.
		await Promise.all(ALL_PORTS.map(p => waitForPortFree(p)));

		// Clean agent app data for a fresh start on setup retries.
		// Skip for verify phase — it needs data from the setup phase.
		if (phase === 'setup') {
			for (const agent of ['agent-1', 'agent-2']) {
				const appData = path.join(ROOT, '.dbs', 'compat', agent, 'studio.darksoil.dashchat');
				try { rmSync(appData, { recursive: true, force: true }); } catch { /* ignore */ }
			}
		}

		tauriDriver1 = spawn(
			'tauri-driver',
			['--port', String(port1), '--native-port', String(nativePort1)],
			{ stdio: ['ignore', 'pipe', 'pipe'] },
		);
		tauriDriver1.stderr?.on('data', (data: Buffer) => {
			console.error(`[tauri-driver:${port1}] ${data.toString().trim()}`);
		});

		tauriDriver2 = spawn(
			'tauri-driver',
			['--port', String(port2), '--native-port', String(nativePort2)],
			{ stdio: ['ignore', 'pipe', 'pipe'] },
		);
		tauriDriver2.stderr?.on('data', (data: Buffer) => {
			console.error(`[tauri-driver:${port2}] ${data.toString().trim()}`);
		});

		// Wait for tauri-driver instances to accept connections.
		await Promise.all([
			waitForPortListening(port1),
			waitForPortListening(port2),
		]);
	},

	async afterSession() {
		// SIGKILL tauri-drivers and wait for exit to free ports.
		await Promise.all([killAndWait(tauriDriver1), killAndWait(tauriDriver2)]);
		// Kill orphaned dash-chat instances and anything holding our ports.
		killAllE2EProcesses();
		killPortHolders(ALL_PORTS);
		// Do NOT clean up .dbs/compat/ — data must persist between setup and verify phases
	},
};
