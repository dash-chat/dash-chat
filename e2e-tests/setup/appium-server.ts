/**
 * The Appium server phone sessions go through. The harness starts it rather
 * than wdio's appium service, in a process group of its own, so the Ctrl-C
 * that stops a run does not stop Appium with it: the workers still need it
 * for the few seconds they spend taking the phones off the test networks
 * (see `setupAgents`). A shell wrapper ends it shortly after the launcher
 * process is gone, however the launcher ended, so it never outlives the run.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { waitForPortListening } from './wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(__dirname, '..');
const ROOT = path.resolve(E2E_DIR, '..');

/** How long Appium outlives the launcher. The launcher waits for its workers
 *  before it exits, so this only has to cover their last calls in flight. */
const OUTLIVE_LAUNCHER_S = 3;

/** Runs Appium (`$0 "$@"`) until it exits or the process `$LAUNCHER_PID` has
 *  been gone for `$OUTLIVE_S` seconds. */
const WRAPPER = `
"$0" "$@" &
appium=$!
while kill -0 "$LAUNCHER_PID" 2>/dev/null && kill -0 "$appium" 2>/dev/null; do
	sleep 1
done
sleep "$OUTLIVE_S"
kill "$appium" 2>/dev/null
wait "$appium"
`;

export async function startAppium(port: number): Promise<ChildProcess> {
	const proc = spawn(
		'sh',
		[
			'-c',
			WRAPPER,
			path.join(E2E_DIR, 'node_modules', '.bin', 'appium'),
			'--port',
			String(port),
			'--log',
			path.join(ROOT, '.dbs', 'e2e', 'appium.log'),
		],
		{
			stdio: 'ignore',
			detached: true,
			env: {
				...process.env,
				LAUNCHER_PID: String(process.pid),
				OUTLIVE_S: String(OUTLIVE_LAUNCHER_S),
				// The checkout-scoped cleanup tells this run's server from another
				// checkout's by this path in its environment.
				E2E_DBS: path.join(ROOT, '.dbs') + path.sep,
			},
		},
	);
	await new Promise<void>((resolve, reject) => {
		proc.once('spawn', resolve);
		proc.once('error', err =>
			reject(new Error(`could not start appium: ${err.message}`)),
		);
	});
	// Appium loads its drivers before it listens.
	await waitForPortListening(port, 60_000);
	console.log(`[appium] ready on port ${port} (pid=${proc.pid})`);
	return proc;
}
