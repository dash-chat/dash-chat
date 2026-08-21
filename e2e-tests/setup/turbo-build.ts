/**
 * The onPrepare app builds run through turbo (see turbo.json at the repo
 * root), which skips a build when nothing changed: inputs are the whole
 * working tree minus e2e-tests/ and docs/ ($TURBO_DEFAULT$ is gitignore-aware
 * and hashes untracked files too) plus the declared env values, and outputs
 * are restored from .turbo/cache on a hit — including healing an output a
 * dev build overwrote, and swapping artifacts when a baked env value flips
 * back to a previously-built one. E2E_FORCE_BUILD=1 rebuilds unconditionally.
 */
import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Run one of the root `e2e:build:*` scripts through turbo with the baked
 *  env. `env` must be the full child environment (turbo runs in loose env
 *  mode; only the vars listed in the task's `env` key affect the hash). */
export function runTurboBuild(task: string, env: NodeJS.ProcessEnv): void {
	const force = (process.env.E2E_FORCE_BUILD ?? '') !== '' ? ' --force' : '';
	execSync(`pnpm exec turbo run ${task} --output-logs=new-only${force}`, {
		cwd: ROOT,
		stdio: 'inherit',
		env: {
			...env,
			TURBO_TELEMETRY_DISABLED: '1',
			TURBO_UI: 'false',
		},
	});
}
