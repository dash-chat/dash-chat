import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

import { UI_TIMEOUT } from './helpers/timeouts';
import { allocateDriverPorts, allocatePort } from './setup/allocate-port';
import {
	killAllE2EProcesses,
	killAndWait,
	killLeftoverMailboxServers,
	killPortHolders,
} from './setup/cleanup';
import { mailboxLogFile, spawnMailboxServer } from './setup/mailbox-server';
import { remoteMailboxUrl } from './setup/test-env';
import { waitForPortFree, waitForPortListening } from './setup/wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

function getSpecFileRetries(): number {
	const rawRetries = process.env.E2E_SPEC_FILE_RETRIES ?? '1';
	const retries = Number.parseInt(rawRetries, 10);
	if (Number.isNaN(retries) || retries < 0) {
		throw new Error(
			`E2E_SPEC_FILE_RETRIES must be a non-negative integer, got ${rawRetries}`,
		);
	}
	return retries;
}

const { port1, nativePort1, port2, nativePort2 } = allocateDriverPorts();
const ALL_PORTS = [port1, nativePort1, port2, nativePort2];

let mailboxServer: ChildProcess;
let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;
let agent1Logger: ChildProcess | null = null;
let agent2Logger: ChildProcess | null = null;
let mailboxLogger: ChildProcess | null = null;

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

export const config: WebdriverIO.MultiremoteConfig = {
	runner: 'local',

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,
	specFileRetries: getSpecFileRetries(),

	capabilities: {
		agent1: {
			port: port1,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'setup', 'launch-agent1.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: port2,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : process.platform,
				'tauri:options': {
					application: path.join(__dirname, 'setup', 'launch-agent2.sh'),
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

	async onPrepare() {
		// Clean up leftover databases from previous interrupted runs
		const dataDir = path.join(ROOT, '.dbs', 'e2e');
		try {
			rmSync(dataDir, { recursive: true, force: true });
		} catch {
			// ignore
		}

		// Kill any leftover processes from previous interrupted runs
		killAllE2EProcesses();
		killLeftoverMailboxServers();
		killPortHolders(ALL_PORTS);

		const mailboxInfoPath = path.join(ROOT, '.dbs', 'e2e', 'mailbox-info.json');

		// When MAILBOX_URL names an allowlisted deployment environment, run
		// against its cloud mailbox instead of spawning a local server. Specs
		// that drive the mailbox's lifecycle skip themselves via
		// isRemoteMailbox().
		const remoteUrl = remoteMailboxUrl();
		if (remoteUrl !== null) {
			process.env.MAILBOX_URL = remoteUrl;
			mkdirSync(path.dirname(mailboxInfoPath), { recursive: true });
			writeFileSync(
				mailboxInfoPath,
				JSON.stringify({ remote: true, url: remoteUrl }),
			);
			console.log(`Using remote mailbox at ${remoteUrl}`);
			return;
		}

		// Start a local mailbox server so e2e tests don't hit the internet.
		const mailboxPort = allocatePort();
		const mailboxUrl = `http://localhost:${mailboxPort}`;
		const mailboxDb = path.join(
			ROOT,
			'.dbs',
			'e2e',
			'mailbox-server',
			'mailbox.db',
		);
		mkdirSync(path.dirname(mailboxDb), { recursive: true });

		console.log(`Starting local mailbox server on ${mailboxUrl}...`);
		// Tail the server's log file (cargo output + the server's tracing, which
		// goes to stdout) and echo it with a prefix, like the agent logs.
		mailboxLogger = startAgentLogger(
			'mailbox-server',
			mailboxLogFile(mailboxDb),
		);
		mailboxServer = spawnMailboxServer(mailboxPort, mailboxDb);
		console.log(`[mailbox-server] spawned (cargo pid=${mailboxServer.pid})`);
		mailboxServer.on('exit', (code, signal) => {
			console.error(
				`[mailbox-server] EXITED code=${code} signal=${signal} at ${new Date().toISOString()}`,
			);
		});
		mailboxServer.on('error', err => {
			console.error(`[mailbox-server] ERROR ${err.message}`);
		});

		// Wait for the mailbox server to be ready.
		const deadline = Date.now() + 30_000;
		let ready = false;
		while (Date.now() < deadline) {
			try {
				execSync(`curl -s ${mailboxUrl}`, { stdio: 'ignore' });
				ready = true;
				break;
			} catch {
				await new Promise(r => setTimeout(r, 1000));
			}
		}
		if (!ready) throw new Error('Mailbox server failed to start');

		// Expose the URL so launch scripts pass it to the Tauri agents.
		process.env.MAILBOX_URL = mailboxUrl;
		console.log(`Mailbox server ready at ${mailboxUrl}`);

		// Persist mailbox info so individual specs can suspend/resume it to
		// drive the offline-UX state transitions.
		writeFileSync(
			mailboxInfoPath,
			JSON.stringify({
				pid: mailboxServer.pid,
				port: mailboxPort,
				url: mailboxUrl,
				dbPath: mailboxDb,
			}),
		);
	},

	async beforeSession() {
		// Force-kill any leftover processes from the previous session.
		await Promise.all([killAndWait(tauriDriver1), killAndWait(tauriDriver2)]);
		killAllE2EProcesses();
		// Kill anything still holding our specific ports (handles orphaned
		// dash-chat processes that inherited tauri-driver's listening sockets).
		killPortHolders(ALL_PORTS);
		// Wait for ports to be fully released after SIGKILL.
		await Promise.all(ALL_PORTS.map(p => waitForPortFree(p)));

		// Clean all agent data for a fresh start (important for specFileRetries).
		// Must remove the entire agent directory, not just the Rust backend data,
		// because WebKitGTK stores localStorage/IndexedDB under the XDG dirs
		// (.local/share/, .config/, .cache/) inside the agent directory.
		for (const agent of ['agent-1', 'agent-2']) {
			const agentDir = path.join(ROOT, '.dbs', 'e2e', agent);
			try {
				rmSync(agentDir, { recursive: true, force: true });
			} catch {
				/* ignore */
			}
		}

		// Tail each agent's stdout/stderr (written by launch-agent.sh) and
		// echo lines to the test runner's stdout with an agent-specific prefix.
		agent1Logger = startAgentLogger(
			'agent-1',
			path.join(ROOT, '.dbs', 'e2e', 'agent-1', 'agent.log'),
		);
		agent2Logger = startAgentLogger(
			'agent-2',
			path.join(ROOT, '.dbs', 'e2e', 'agent-2', 'agent.log'),
		);

		tauriDriver1 = spawn(
			'tauri-driver',
			['--port', String(port1), '--native-port', String(nativePort1)],
			{ stdio: ['ignore', 'ignore', 'pipe'] },
		);
		tauriDriver1.stderr?.on('data', (data: Buffer) => {
			console.error(`[tauri-driver:${port1}] ${data.toString().trim()}`);
		});

		tauriDriver2 = spawn(
			'tauri-driver',
			['--port', String(port2), '--native-port', String(nativePort2)],
			{ stdio: ['ignore', 'ignore', 'pipe'] },
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
		// Kill orphaned dash-chat E2E instances and anything holding our ports.
		killAllE2EProcesses();
		killPortHolders(ALL_PORTS);
		agent1Logger?.kill();
		agent2Logger?.kill();
		agent1Logger = null;
		agent2Logger = null;
	},

	onComplete() {
		if (mailboxServer?.pid) {
			// Negative PID = signal the entire process group, so we reach the
			// mailbox-server child that `cargo run` spawned underneath.
			try {
				process.kill(-mailboxServer.pid, 'SIGTERM');
			} catch {
				/* already gone */
			}
		}
		killAllE2EProcesses();
		killPortHolders(ALL_PORTS);
		agent1Logger?.kill();
		agent2Logger?.kill();
		mailboxLogger?.kill();
	},
};
