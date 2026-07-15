import { type ChildProcess, spawn } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

import { UI_TIMEOUT } from '../helpers/timeouts';
import { allocatePinnedPort } from '../setup/allocate-port';
import {
	killAllE2EProcesses,
	killAndWait,
	killPortHolders,
} from '../setup/cleanup';
import { getSpecFileRetries } from '../setup/test-env';
import { waitForPortFree, waitForPortListening } from '../setup/wait-for-port';

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

const port1 = allocatePinnedPort('_WDIO_PORT1');
const nativePort1 = allocatePinnedPort('_WDIO_NATIVE_PORT1');
const port2 = allocatePinnedPort('_WDIO_PORT2');
const nativePort2 = allocatePinnedPort('_WDIO_NATIVE_PORT2');
const ALL_PORTS = [port1, nativePort1, port2, nativePort2];

let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;
let agent1Logger: ChildProcess | null = null;
let agent2Logger: ChildProcess | null = null;

function startAgentLogger(agent: string, logFile: string): ChildProcess {
	mkdirSync(path.dirname(logFile), { recursive: true });
	writeFileSync(logFile, '');
	const proc = spawn('tail', ['-n', '0', '-F', logFile], {
		stdio: ['ignore', 'pipe', 'ignore'],
	});
	const rl = createInterface({ input: proc.stdout! });
	rl.on('line', (line: string) => {
		console.log(`[${agent}] ${line}`);
	});
	return proc;
}

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: [specFile],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: {
		agent1: {
			port: port1,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'scripts', 'launch-agent1.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: port2,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'scripts', 'launch-agent2.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
	},

	logLevel: 'warn',
	waitforTimeout: UI_TIMEOUT,

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

		// Clean agent app data for a fresh start on setup retries. The Tauri
		// agent stores its DB under $DATA_DIR/<version>/ and WebKitGTK puts
		// localStorage/IndexedDB under XDG dirs inside $DATA_DIR
		// (.local/share/, .config/, .cache/), so we must wipe the whole dir,
		// not just an `studio.darksoil.dashchat` subpath that doesn't exist.
		// Skip for verify phase — it needs data from the setup phase.
		if (phase === 'setup') {
			for (const agent of ['agent-1', 'agent-2']) {
				const agentDir = path.join(ROOT, '.dbs', 'compat', agent);
				try {
					rmSync(agentDir, { recursive: true, force: true });
				} catch {
					/* ignore */
				}
			}
		}

		// Tail each agent's stdout/stderr (written by launch-agent.sh) and
		// echo lines to the test runner's stdout with an agent-specific prefix.
		agent1Logger = startAgentLogger(
			'agent-1',
			path.join(ROOT, '.dbs', 'compat', 'agent-1', 'agent.log'),
		);
		agent2Logger = startAgentLogger(
			'agent-2',
			path.join(ROOT, '.dbs', 'compat', 'agent-2', 'agent.log'),
		);

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
		agent1Logger?.kill();
		agent2Logger?.kill();
		agent1Logger = null;
		agent2Logger = null;
		// Do NOT clean up .dbs/compat/ — data must persist between setup and verify phases
	},
};
