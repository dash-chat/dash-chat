import type { Options } from '@wdio/types';
import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { allocateDriverPorts, allocatePort } from './helpers/allocate-port';
import {
	killAndWait,
	killAllE2EProcesses,
	killLeftoverMailboxServers,
	killPortHolders,
} from './helpers/cleanup';
import { waitForPortFree, waitForPortListening } from './helpers/wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const { port1, nativePort1, port2, nativePort2 } = allocateDriverPorts();
const ALL_PORTS = [port1, nativePort1, port2, nativePort2];

let mailboxServer: ChildProcess;
let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;

export const config: Options.Testrunner = {
	runner: 'local',
	tsNodeOpts: { esm: true, project: path.join(__dirname, 'tsconfig.json') },

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,
	specFileRetries: 1,

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
	waitforTimeout: 10_000,

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

		if (!process.env.SKIP_BUILD) {
			console.log('Building Tauri app (debug, no-bundle)...');
			execSync('pnpm tauri build --debug --no-bundle --features e2e-tests', {
				cwd: ROOT,
				stdio: 'inherit',
			});
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
		mailboxServer = spawn(
			'cargo',
			[
				'run',
				'-p',
				'mailbox-server',
				'--',
				'--db-path',
				mailboxDb,
				'--addr',
				`0.0.0.0:${mailboxPort}`,
			],
			{ cwd: ROOT, stdio: ['ignore', 'ignore', 'pipe'] },
		);
		mailboxServer.stderr?.on('data', (data: Buffer) => {
			console.error(`[mailbox-server] ${data.toString().trim()}`);
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

		// Clean agent app data for a fresh start (important for specFileRetries).
		for (const agent of ['agent-1', 'agent-2']) {
			const appData = path.join(
				ROOT,
				'.dbs',
				'e2e',
				agent,
				'studio.darksoil.dashchat',
			);
			try {
				rmSync(appData, { recursive: true, force: true });
			} catch {
				/* ignore */
			}
		}

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
	},

	onComplete() {
		if (mailboxServer) mailboxServer.kill();
		killAllE2EProcesses();
		killPortHolders(ALL_PORTS);
	},
};
