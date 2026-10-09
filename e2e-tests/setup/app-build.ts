import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Run one of the root `e2e:build:*` scripts with the baked env. `env` must
 *  be the full child environment. */
export function runAppBuild(task: string, env: NodeJS.ProcessEnv): void {
	execSync(`pnpm ${task}`, { cwd: ROOT, stdio: 'inherit', env });
}
