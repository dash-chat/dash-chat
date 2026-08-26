/**
 * Mocha root hook plugin (loaded via `mochaOpts.require`) that fails any test
 * during which an agent's app hit an uncaught error or unhandled rejection —
 * the app surfaces every one of those to the user as an unexpected-error
 * toast, so no spec may pass while one occurs.
 *
 * Detection reads the per-agent device logs the harness captures to
 * `.dbs/e2e/agents/agent-<slot>.log`: the app's global handlers in
 * `ui/src/lib/utils/logs.ts` log every such error with an
 * `[unhandledrejection]` / `[uncaught]` marker, and the files outlive the app
 * process — so this also catches errors thrown while the app is shutting
 * itself down (e.g. account deletion), which no webview-side check could see.
 *
 * Log lines are only attributed to a test if they land while it runs: a
 * `beforeEach` fast-forward discards suite-setup noise (e.g. the iOS
 * reset-to-first-launch path, which exercises the same shutdown code).
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const AGENT_LOG_DIR = path.resolve(
	__dirname,
	'..',
	'..',
	'.dbs',
	'e2e',
	'agents',
);

const UNCAUGHT_MARKERS = /\[unhandledrejection\]|\[uncaught\]/;

/** Byte offset already scanned per log file. */
const scannedUpTo = new Map<string, number>();

function agentLogFiles(): string[] {
	try {
		return readdirSync(AGENT_LOG_DIR)
			.filter(name => /^agent-\d+\.log$/.test(name))
			.map(name => path.join(AGENT_LOG_DIR, name));
	} catch {
		return [];
	}
}

/** Advance every log's scan offset to its current end, returning the
 *  uncaught-error lines that were skipped over. */
function scanLogs(): string[] {
	const errors: string[] = [];
	for (const file of agentLogFiles()) {
		const from = scannedUpTo.get(file) ?? statSync(file).size;
		const size = statSync(file).size;
		scannedUpTo.set(file, size);
		if (size <= from) continue;
		const chunk = readFileSync(file).subarray(from, size).toString('utf8');
		for (const line of chunk.split('\n')) {
			if (UNCAUGHT_MARKERS.test(line)) {
				errors.push(`${path.basename(file)}: ${line}`);
			}
		}
	}
	return errors;
}

function assertNoUncaughtErrors(errors: string[], when: string): void {
	if (errors.length > 0) {
		throw new Error(
			`the app hit uncaught errors ${when}:\n${errors.join('\n')}`,
		);
	}
}

export const mochaHooks = {
	beforeEach() {
		// A fresh agent log file may appear between tests (first scan of this
		// worker, or an agent launched mid-suite); scanLogs starts unseen files
		// at their current end, so setup noise never reaches a test.
		scanLogs();
	},
	afterEach() {
		assertNoUncaughtErrors(scanLogs(), 'during this test');
	},
	async afterAll() {
		// Device logs reach the capture files with some latency (syslog/logcat
		// tailing), so give lines from the final test a moment to land.
		await new Promise(resolve => setTimeout(resolve, 2000));
		assertNoUncaughtErrors(scanLogs(), 'at the end of this spec file');
	},
};
