import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

import { allocatePort } from './setup/allocate-port';
import {
	killAndWait,
	killAllE2EProcesses,
	killLeftoverMailboxServers,
	killPortHolders,
} from './setup/cleanup';
import { waitForPortFree, waitForPortListening } from './setup/wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const MAILBOX_BIN = path.join(ROOT, 'target', 'debug', 'mailbox-server');

// Number of multiremote workers to run in parallel. Each worker drives its own
// pair of agents + a private mailbox server on disjoint ports.
const MAX_INSTANCES = Number(process.env.E2E_MAX_INSTANCES ?? '2');

interface WorkerResources {
	workerId: string;
	dataDir: string;
	driverPort1: number;
	nativePort1: number;
	driverPort2: number;
	nativePort2: number;
	mailboxPort: number;
	mailboxServer: ChildProcess;
	tauriDriver1: ChildProcess;
	tauriDriver2: ChildProcess;
	agent1Logger: ChildProcess | null;
	agent2Logger: ChildProcess | null;
}

// Each worker is its own Node process, so this Map is naturally per-worker.
const workerResources = new Map<string, WorkerResources>();

function startAgentLogger(agent: string, logFile: string): ChildProcess {
	// Pre-create the log file so `tail` doesn't error before the agent boots.
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

async function spawnWorkerMailbox(
	workerId: string,
	dataDir: string,
): Promise<{ proc: ChildProcess; port: number; url: string; dbPath: string }> {
	const port = allocatePort();
	const url = `http://localhost:${port}`;
	const dbPath = path.join(dataDir, 'mailbox-server', 'mailbox.db');
	mkdirSync(path.dirname(dbPath), { recursive: true });

	console.log(`[worker ${workerId}] starting mailbox server on ${url}...`);
	// detached:true puts the mailbox in its own process group so we can SIGSTOP
	// the whole group from inside specs to drive offline-UX transitions.
	const proc = spawn(
		MAILBOX_BIN,
		['--db-path', dbPath, '--addr', `0.0.0.0:${port}`],
		{ cwd: ROOT, stdio: ['ignore', 'ignore', 'pipe'], detached: true },
	);
	console.log(`[worker ${workerId}] mailbox spawned (pid=${proc.pid})`);
	proc.stderr?.on('data', (data: Buffer) => {
		console.error(
			`[worker ${workerId}][mailbox-server] ${data.toString().trim()}`,
		);
	});
	proc.on('exit', (code, signal) => {
		console.error(
			`[worker ${workerId}][mailbox-server] EXITED code=${code} signal=${signal}`,
		);
	});

	const deadline = Date.now() + 30_000;
	let ready = false;
	while (Date.now() < deadline) {
		try {
			execSync(`curl -s ${url}`, { stdio: 'ignore' });
			ready = true;
			break;
		} catch {
			await new Promise(r => setTimeout(r, 250));
		}
	}
	if (!ready) throw new Error(`Mailbox server for worker ${workerId} failed to start`);

	return { proc, port, url, dbPath };
}

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: MAX_INSTANCES,
	specFileRetries: 1,

	// Placeholder ports — overwritten per-worker in beforeSession before WDIO
	// initialises sessions. Each worker rewrites these to its own free ports so
	// parallel workers don't collide.
	capabilities: {
		agent1: {
			port: 0,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'setup', 'launch-agent1.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: 0,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'setup', 'launch-agent2.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
	},

	logLevel: 'warn',
	waitforTimeout: 10_000,

	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 120_000,
	},

	reporters: ['spec'],

	async onPrepare() {
		// Aggressive global cleanup — we know nothing of ours should be running.
		killAllE2EProcesses();
		killLeftoverMailboxServers();

		// Wipe the whole e2e data tree once; per-worker subdirs are recreated
		// inside beforeSession.
		const dataDir = path.join(ROOT, '.dbs', 'e2e');
		try {
			rmSync(dataDir, { recursive: true, force: true });
		} catch {
			/* ignore */
		}

		// Build the standalone mailbox-server binary once so workers can exec it
		// directly instead of paying the `cargo run` lock-contention tax each
		// time. Skip the rebuild if the binary already exists (it's a debug
		// build, cargo would no-op anyway, but this saves the cargo invocation).
		if (!existsSync(MAILBOX_BIN)) {
			console.log('Building mailbox-server binary...');
			execSync('cargo build -p mailbox-server', {
				cwd: ROOT,
				stdio: 'inherit',
			});
		}
	},

	async beforeSession(_config, capabilities, _specs, cid) {
		const workerId = String(cid ?? `pid-${process.pid}`);
		const workerDir = path.join(ROOT, '.dbs', 'e2e', `worker-${workerId}`);

		// Allocate disjoint ports per worker.
		const driverPort1 = allocatePort();
		const nativePort1 = allocatePort();
		const driverPort2 = allocatePort();
		const nativePort2 = allocatePort();
		const workerPorts = [driverPort1, nativePort1, driverPort2, nativePort2];

		// Mutate capabilities so WDIO connects to the right tauri-driver per
		// browser. In multiremote, capabilities is a Record<name, capability>;
		// the `.port` field is read AFTER beforeSession.
		const caps = capabilities as Record<string, { port?: number }>;
		caps.agent1.port = driverPort1;
		caps.agent2.port = driverPort2;

		// Per-worker mailbox server.
		const mailbox = await spawnWorkerMailbox(workerId, workerDir);

		// Per-worker mailbox-info JSON. mailbox-control.ts reads
		// E2E_MAILBOX_INFO_PATH to find this file.
		const mailboxInfoPath = path.join(workerDir, 'mailbox-info.json');
		mkdirSync(path.dirname(mailboxInfoPath), { recursive: true });
		writeFileSync(
			mailboxInfoPath,
			JSON.stringify({
				pid: mailbox.proc.pid,
				port: mailbox.port,
				url: mailbox.url,
				dbPath: mailbox.dbPath,
			}),
		);

		// Env vars consumed by launch-agent.sh + mailbox-control.ts. These flow
		// into all child processes this worker spawns.
		process.env.MAILBOX_URL = mailbox.url;
		process.env.E2E_WORKER_ID = workerId;
		process.env.E2E_MAILBOX_INFO_PATH = mailboxInfoPath;

		// Make sure ports are free (in case a prior crashed run leaked).
		killPortHolders(workerPorts);
		await Promise.all(workerPorts.map(p => waitForPortFree(p)));

		// Per-worker agent log tails.
		const agent1Logger = startAgentLogger(
			`worker-${workerId}/agent-1`,
			path.join(workerDir, 'agent-1', 'agent.log'),
		);
		const agent2Logger = startAgentLogger(
			`worker-${workerId}/agent-2`,
			path.join(workerDir, 'agent-2', 'agent.log'),
		);

		const tauriDriver1 = spawn(
			'tauri-driver',
			['--port', String(driverPort1), '--native-port', String(nativePort1)],
			{ stdio: ['ignore', 'ignore', 'pipe'] },
		);
		tauriDriver1.stderr?.on('data', (data: Buffer) => {
			console.error(
				`[worker ${workerId}][tauri-driver:${driverPort1}] ${data.toString().trim()}`,
			);
		});

		const tauriDriver2 = spawn(
			'tauri-driver',
			['--port', String(driverPort2), '--native-port', String(nativePort2)],
			{ stdio: ['ignore', 'ignore', 'pipe'] },
		);
		tauriDriver2.stderr?.on('data', (data: Buffer) => {
			console.error(
				`[worker ${workerId}][tauri-driver:${driverPort2}] ${data.toString().trim()}`,
			);
		});

		await Promise.all([
			waitForPortListening(driverPort1),
			waitForPortListening(driverPort2),
		]);

		workerResources.set(workerId, {
			workerId,
			dataDir: workerDir,
			driverPort1,
			nativePort1,
			driverPort2,
			nativePort2,
			mailboxPort: mailbox.port,
			mailboxServer: mailbox.proc,
			tauriDriver1,
			tauriDriver2,
			agent1Logger,
			agent2Logger,
		});
	},

	async afterSession(_config, _capabilities, _specs) {
		const workerId = process.env.E2E_WORKER_ID;
		if (!workerId) return;
		const res = workerResources.get(workerId);
		if (!res) return;

		// Kill drivers first so they release their ports.
		await Promise.all([
			killAndWait(res.tauriDriver1),
			killAndWait(res.tauriDriver2),
		]);

		// Reap this worker's dash-chat processes; do NOT touch peer workers.
		killAllE2EProcesses(`worker-${workerId}`);

		// Then the mailbox process group.
		if (res.mailboxServer.pid && res.mailboxServer.exitCode === null) {
			try {
				process.kill(-res.mailboxServer.pid, 'SIGTERM');
			} catch {
				/* already gone */
			}
		}

		killPortHolders([
			res.driverPort1,
			res.nativePort1,
			res.driverPort2,
			res.nativePort2,
			res.mailboxPort,
		]);

		res.agent1Logger?.kill();
		res.agent2Logger?.kill();

		workerResources.delete(workerId);
	},

	onComplete() {
		// Final sweep in the launcher in case workers crashed before afterSession.
		killAllE2EProcesses();
		killLeftoverMailboxServers();
	},
};
