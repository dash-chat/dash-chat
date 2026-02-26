import { type ChildProcess, execSync, spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';
import { allocateDriverPorts } from './helpers/allocate-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const { port1, nativePort1, port2, nativePort2 } = allocateDriverPorts();

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

	onPrepare() {
		if (process.env.SKIP_BUILD) return;

		console.log('Building Tauri app (debug, no-bundle)...');
		execSync('pnpm tauri build --debug --no-bundle', {
			cwd: ROOT,
			stdio: 'inherit',
			env: { ...process.env, VITE_E2E: '1' },
		});
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
};
