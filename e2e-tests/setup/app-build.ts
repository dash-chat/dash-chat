import { execSync, spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { envWithoutWdioLoader } from './harness-env';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Passed to every `tauri build`: the harness builds the frontend once per
 *  run (`buildFrontend`), so tauri's own frontend step is skipped. */
export const SKIP_FRONTEND_BUILD = `--config '{"build":{"beforeBuildCommand":""}}'`;

/** Run a build command from the repo root. `env` must be the full child
 *  environment, with the values the build bakes in. */
export function runBuild(command: string, env: NodeJS.ProcessEnv): void {
	execSync(command, { cwd: ROOT, stdio: 'inherit', env });
}

/** The frontend build tauri.conf.json would run before each platform build,
 *  run once with the e2e flag; resolves when it has finished. */
export function buildFrontend(): Promise<void> {
	const { build } = JSON.parse(
		readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'),
	) as { build: { beforeBuildCommand: string } };
	return new Promise<void>((resolve, reject) => {
		const proc = spawn(build.beforeBuildCommand, {
			cwd: ROOT,
			stdio: 'inherit',
			shell: true,
			env: envWithoutWdioLoader({ VITE_E2E: 'true' }),
		});
		proc.on('error', reject);
		proc.on('exit', code => {
			if (code === 0) resolve();
			else reject(new Error(`frontend build exited ${code}`));
		});
	});
}
