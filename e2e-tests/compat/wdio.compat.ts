import type { ChildProcess } from 'node:child_process';
import { closeSync, mkdirSync, openSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { UI_TIMEOUT } from '../helpers/timeouts';
import { startAgentLogger } from '../setup/agent-logger';
import { allocatePinnedPort } from '../setup/allocate-port';
import {
	killAllE2EProcesses,
	killAndWait,
	killPortHolders,
} from '../setup/cleanup';
import { launchDesktopApp } from '../setup/platforms/desktop';
import { getSpecFileRetries } from '../setup/test-env';
import { waitForPortFree } from '../setup/wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(__dirname, '..');
const ROOT = path.resolve(__dirname, '../..');

const phase = process.env.COMPAT_PHASE;
if (!phase || !['setup', 'verify'].includes(phase)) {
	throw new Error('COMPAT_PHASE must be "setup" or "verify"');
}

function requiredEnv(name: string): string {
	const value = process.env[name];
	if (value === undefined) throw new Error(`${name} env var required`);
	return value;
}

const compatBinary = requiredEnv('COMPAT_BINARY');

const specFile =
	phase === 'setup'
		? path.join(E2E_DIR, 'specs', 'compat-setup.spec.ts')
		: path.join(E2E_DIR, 'specs', 'compat-verify.spec.ts');

const port1 = allocatePinnedPort('_WDIO_PORT1');
const port2 = allocatePinnedPort('_WDIO_PORT2');
const ALL_PORTS = [port1, port2];

const PLATFORM_NAME = process.platform === 'darwin' ? 'mac' : 'linux';

function agentDir(agent: number): string {
	return path.join(ROOT, '.dbs', 'compat', `agent-${agent}`);
}

function agentLogPath(agent: number): string {
	return path.join(agentDir(agent), 'agent.log');
}

/** Launch the compat binary for `agent`, its stdout and stderr going to the
 *  log the runner tails; truncated per launch so retries start fresh. */
async function launchAgent(agent: number, port: number): Promise<ChildProcess> {
	mkdirSync(agentDir(agent), { recursive: true });
	const log = openSync(agentLogPath(agent), 'w');
	const app = await launchDesktopApp(compatBinary, agentDir(agent), port, {}, [
		'ignore',
		log,
		log,
	]);
	closeSync(log);
	return app;
}

let app1: ChildProcess | undefined;
let app2: ChildProcess | undefined;
let agent1Logger: ChildProcess | null = null;
let agent2Logger: ChildProcess | null = null;

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: [specFile],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: {
		agent1: {
			port: port1,
			capabilities: {
				platformName: PLATFORM_NAME,
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: port2,
			capabilities: {
				platformName: PLATFORM_NAME,
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
		await Promise.all([killAndWait(app1), killAndWait(app2)]);
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
			for (const agent of [1, 2]) {
				try {
					rmSync(agentDir(agent), { recursive: true, force: true });
				} catch {
					/* ignore */
				}
			}
		}

		// Tail each agent's stdout/stderr and echo lines to the test runner's
		// stdout with an agent-specific prefix.
		agent1Logger = startAgentLogger('agent-1', agentLogPath(1));
		agent2Logger = startAgentLogger('agent-2', agentLogPath(2));

		[app1, app2] = await Promise.all([
			launchAgent(1, port1),
			launchAgent(2, port2),
		]);
	},

	async afterSession() {
		// SIGKILL the apps and wait for exit to free ports.
		await Promise.all([killAndWait(app1), killAndWait(app2)]);
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
