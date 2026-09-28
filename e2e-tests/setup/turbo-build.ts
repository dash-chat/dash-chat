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
 *  mode; only the vars listed in the task's `env` key affect the hash).
 *  Returns whether the build ran: on a cache hit the artifacts are last
 *  build's, which is what the sources say they should be, and nothing a
 *  packaging step could have got wrong happened this time. */
export function runTurboBuild(task: string, env: NodeJS.ProcessEnv): boolean {
	const turboEnv = {
		...env,
		TURBO_TELEMETRY_DISABLED: '1',
		TURBO_UI: 'false',
	};
	const force = (process.env.E2E_FORCE_BUILD ?? '') !== '' ? ' --force' : '';
	const willRun = force !== '' || cacheStatus(task, turboEnv) !== 'HIT';
	execSync(`pnpm exec turbo run ${task} --output-logs=new-only${force}`, {
		cwd: ROOT,
		stdio: 'inherit',
		env: turboEnv,
	});
	return willRun;
}

interface TurboDryRun {
	tasks?: { cache?: { status?: string } }[];
}

/** What turbo says it would do with `task`, asked before doing it: its own
 *  run output only says so once the build is over. */
function cacheStatus(task: string, env: NodeJS.ProcessEnv): string | null {
	try {
		const dry: TurboDryRun = JSON.parse(
			execSync(`pnpm exec turbo run ${task} --dry=json`, {
				cwd: ROOT,
				encoding: 'utf8',
				env,
			}),
		);
		return dry.tasks?.[0]?.cache?.status ?? null;
	} catch {
		// Never let asking stop the build: an unreadable answer just means the
		// APK is checked as if it had been built.
		return null;
	}
}
