import { type ChildProcess, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(__dirname, '..');

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

let tauriDriver1: ChildProcess;
let tauriDriver2: ChildProcess;

export const config: Options.Testrunner = {
	runner: 'local',
	tsNodeOpts: { esm: true, project: path.join(E2E_DIR, 'tsconfig.json') },

	specs: [specFile],
	maxInstances: 1,

	capabilities: {
		agent1: {
			port: 4444,
			capabilities: {
				'platformName': 'linux',
				'tauri:options': {
					application: path.join(__dirname, 'scripts', 'launch-agent1.sh'),
				},
			} as WebdriverIO.Capabilities,
		},
		agent2: {
			port: 4446,
			capabilities: {
				'platformName': 'linux',
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

	beforeSession() {
		tauriDriver1 = spawn(
			'tauri-driver',
			['--port', '4444', '--native-port', '4445'],
			{ stdio: ['ignore', 'pipe', 'pipe'] },
		);
		tauriDriver1.stderr?.on('data', (data: Buffer) => {
			console.error(`[tauri-driver:4444] ${data.toString().trim()}`);
		});

		tauriDriver2 = spawn(
			'tauri-driver',
			['--port', '4446', '--native-port', '4447'],
			{ stdio: ['ignore', 'pipe', 'pipe'] },
		);
		tauriDriver2.stderr?.on('data', (data: Buffer) => {
			console.error(`[tauri-driver:4446] ${data.toString().trim()}`);
		});

		return new Promise((resolve) => setTimeout(resolve, 500));
	},

	afterSession() {
		if (tauriDriver1) tauriDriver1.kill();
		if (tauriDriver2) tauriDriver2.kill();
		// Do NOT clean up .dbs/compat/ — data must persist between setup and verify phases
	},
};
