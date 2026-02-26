import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

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

	onPrepare() {
		if (process.env.SKIP_BUILD) return;

		console.log('Building Tauri app (debug, no-bundle)...');
		execSync('pnpm tauri build --debug --no-bundle', {
			cwd: ROOT,
			stdio: 'inherit',
		});
	},

	beforeSession() {
		// Spawn two tauri-driver instances on different ports
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

		// Give drivers a moment to start
		return new Promise((resolve) => setTimeout(resolve, 500));
	},

	afterSession() {
		if (tauriDriver1) tauriDriver1.kill();
		if (tauriDriver2) tauriDriver2.kill();

		// Clean up E2E data directories
		const dataDir = path.join(ROOT, '.dbs', 'e2e');
		try {
			rmSync(dataDir, { recursive: true, force: true });
		} catch {
			// ignore
		}
	},
};
