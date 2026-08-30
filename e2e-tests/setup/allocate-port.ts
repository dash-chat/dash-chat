import { execSync } from 'node:child_process';
import { type AddressInfo, createServer } from 'node:net';

/** Allocate a free TCP port by briefly binding to port 0. Sync (a subprocess
 *  does the binding) because config-load-time callers can't await. */
function allocatePort(): number {
	return Number(
		execSync(
			'node -e "const s=require(\'net\').createServer();s.listen(0,()=>{process.stdout.write(String(s.address().port));s.close()})"',
		)
			.toString()
			.trim(),
	);
}

/** The port a briefly-bound server on `port` actually got (0 = any free
 *  port), or null when it can't be bound. */
function tryBind(port: number): Promise<number | null> {
	return new Promise(resolve => {
		const server = createServer();
		server.once('error', () => resolve(null));
		server.listen(port, () => {
			const bound = (server.address() as AddressInfo).port;
			server.close(() => resolve(bound));
		});
	});
}

/**
 * The preferred port when it is free, a random free one otherwise. Servers
 * whose URL gets baked into an app build (the iOS mailbox/push URLs) use this
 * so an unchanged build keeps pointing at a live server across runs — a
 * different port every run would force a rebuild every run.
 */
export async function allocatePreferredPort(
	preferred: number,
): Promise<number> {
	return (await tryBind(preferred)) ?? (await tryBind(0))!;
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
