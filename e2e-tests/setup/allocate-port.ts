import { execSync } from 'node:child_process';

/** Allocate a free TCP port by briefly binding to port 0. */
export function allocatePort(): number {
	return Number(
		execSync(
			'node -e "const s=require(\'net\').createServer();s.listen(0,()=>{process.stdout.write(String(s.address().port));s.close()})"',
		)
			.toString()
			.trim(),
	);
}

/**
 * Allocate a port once and pin it via an env var.
 *
 * WDIO's main process and worker process both load the config file
 * independently. The first load (main process) allocates the port and stores
 * it in the env var. The worker inherits the env var and reads the same
 * port, keeping capabilities and beforeSession in sync.
 */
export function allocatePinnedPort(envName: string): number {
	if (process.env[envName] === undefined) {
		process.env[envName] = String(allocatePort());
	}
	return Number(process.env[envName]);
}
