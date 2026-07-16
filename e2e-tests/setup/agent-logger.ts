import { type ChildProcess, spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';

/**
 * Tail a log file and echo lines to the test runner's stdout with a
 * source-specific prefix.
 */
export function startAgentLogger(agent: string, logFile: string): ChildProcess {
	// Pre-create the log file so `tail` doesn't error before the agent boots.
	mkdirSync(path.dirname(logFile), { recursive: true });
	writeFileSync(logFile, '');

	const proc = spawn('tail', ['-n', '0', '-F', logFile], {
		stdio: ['ignore', 'pipe', 'ignore'],
	});
	const rl = createInterface({ input: proc.stdout! });
	rl.on('line', (line: string) => {
		console.log(`[${agent}] ${line}`);
	});
	return proc;
}
