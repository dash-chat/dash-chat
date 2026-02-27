import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';
import { allocateDriverPorts, allocatePort } from './helpers/allocate-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const { port1, nativePort1, port2, nativePort2 } = allocateDriverPorts();

let mailboxServer: ChildProcess;
let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;

export const config: Options.Testrunner = {
	runner: 'local',
	tsNodeOpts: { esm: true, project: path.join(__dirname, 'tsconfig.json') },

	specs: ['./specs/**/*.spec.ts'],
	exclude: ['./specs/compat-*.spec.ts'],
	maxInstances: 1,

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

	async onPrepare() {
		// Clean up leftover databases from previous interrupted runs
		const dataDir = path.join(ROOT, '.dbs', 'e2e');
		try {
			rmSync(dataDir, { recursive: true, force: true });
		} catch {
			// ignore
		}

		if (!process.env.SKIP_BUILD) {
			console.log('Building Tauri app (debug, no-bundle)...');
			execSync('pnpm tauri build --debug --no-bundle', {
				cwd: ROOT,
				stdio: 'inherit',
				env: { ...process.env, VITE_E2E: '1' },
			});
		}

		// Start a local mailbox server so e2e tests don't hit the internet.
		const mailboxPort = allocatePort();
		const mailboxUrl = `http://localhost:${mailboxPort}`;
		const mailboxDb = path.join(ROOT, '.dbs', 'e2e', 'mailbox-server', 'mailbox.db');
		mkdirSync(path.dirname(mailboxDb), { recursive: true });

		console.log(`Starting local mailbox server on ${mailboxUrl}...`);
		mailboxServer = spawn(
			'cargo',
			['run', '-p', 'mailbox-server', '--', '--db-path', mailboxDb, '--addr', `0.0.0.0:${mailboxPort}`],
			{ cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] },
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
				await new Promise((r) => setTimeout(r, 1000));
			}
		}
		if (!ready) throw new Error('Mailbox server failed to start');

		// Expose the URL so launch scripts pass it to the Tauri agents.
		process.env.MAILBOX_URL = mailboxUrl;
		console.log(`Mailbox server ready at ${mailboxUrl}`);
	},

	beforeSession() {
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

		return new Promise((resolve) => setTimeout(resolve, 500));
	},

	afterSession() {
		if (tauriDriver1) tauriDriver1.kill();
		if (tauriDriver2) tauriDriver2.kill();

		const dataDir = path.join(ROOT, '.dbs', 'e2e');
		try {
			rmSync(dataDir, { recursive: true, force: true });
		} catch {
			// ignore
		}
	},

	onComplete() {
		if (mailboxServer) mailboxServer.kill();
	},
};
