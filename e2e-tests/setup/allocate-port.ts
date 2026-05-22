import { execSync } from 'node:child_process';

/** Allocate a free TCP port by briefly binding to port 0. */
export function allocatePort(): number {
	return Number(
		execSync(
			"node -e \"const s=require('net').createServer();s.listen(0,()=>{process.stdout.write(String(s.address().port));s.close()})\"",
		)
			.toString()
			.trim(),
	);
}

/**
 * Allocate 4 ports for a two-agent WDIO setup and pin them via env vars.
 *
 * WDIO's main process and worker process both load the config file
 * independently. The first load (main process) allocates ports and stores
 * them in env vars. The worker inherits those env vars and reads the
 * same ports, keeping capabilities and beforeSession in sync.
 */
export function allocateDriverPorts(): {
	port1: number;
	nativePort1: number;
	port2: number;
	nativePort2: number;
} {
	if (!process.env._WDIO_PORT1) {
		process.env._WDIO_PORT1 = String(allocatePort());
		process.env._WDIO_NATIVE_PORT1 = String(allocatePort());
		process.env._WDIO_PORT2 = String(allocatePort());
		process.env._WDIO_NATIVE_PORT2 = String(allocatePort());
	}
	return {
		port1: Number(process.env._WDIO_PORT1),
		nativePort1: Number(process.env._WDIO_NATIVE_PORT1),
		port2: Number(process.env._WDIO_PORT2),
		nativePort2: Number(process.env._WDIO_NATIVE_PORT2),
	};
}
