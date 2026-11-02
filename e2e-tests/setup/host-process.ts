/**
 * What the harness asks of the host's processes, on POSIX and on Windows, which
 * has no process groups or signals. A Windows "group" is a process and its
 * descendants, which is also what takes an agent's WebView2 processes with it.
 */
import { execFileSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const WINDOWS = process.platform === 'win32';

/** stunnel.exe is the GUI build, which never writes to its log's stdio. */
export const STUNNEL_BINARY = WINDOWS ? 'tstunnel' : 'stunnel';

/** The file name of the executable called `name` on this host. */
export function exe(name: string): string {
	return WINDOWS ? `${name}.exe` : name;
}

const REMOVE_DIR_ATTEMPTS = 50;
const REMOVE_DIR_RETRY_MS = 200;

/** Delete `dir`, waiting out a just-killed process that still holds its files.
 *  rmSync's own maxRetries doesn't cover the EPERM Windows reports for that. */
export function removeDirWithRetries(dir: string): void {
	for (let attempt = 1; ; attempt++) {
		try {
			rmSync(dir, { recursive: true, force: true });
			return;
		} catch (err) {
			if (attempt === REMOVE_DIR_ATTEMPTS) throw err;
			Atomics.wait(
				new Int32Array(new SharedArrayBuffer(4)),
				0,
				0,
				REMOVE_DIR_RETRY_MS,
			);
		}
	}
}

export function isOnPath(tool: string): boolean {
	try {
		if (WINDOWS) execFileSync('where', [tool], { stdio: 'ignore' });
		else execFileSync('sh', ['-c', `command -v ${tool}`], { stdio: 'ignore' });
		return true;
	} catch {
		return false;
	}
}

function runScript(script: string, args: string[]): string {
	return execFileSync(
		'powershell',
		[
			'-NoProfile',
			'-NonInteractive',
			'-ExecutionPolicy',
			'Bypass',
			'-File',
			path.join(__dirname, 'windows', script),
			...args,
		],
		{ encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
	);
}

function killTreeOnWindows(pid: number): void {
	execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], {
		stdio: 'ignore',
	});
}

/** Kill `pid` outright. Throws when it is already gone. */
export function forceKill(pid: number): void {
	if (WINDOWS) killTreeOnWindows(pid);
	else process.kill(pid, 'SIGKILL');
}

/** Signal the process group a `detached` spawn led. Throws when it is gone. */
export function signalGroup(pid: number, signal: NodeJS.Signals): void {
	if (!WINDOWS) {
		process.kill(-pid, signal);
		return;
	}
	if (signal === 'SIGSTOP' || signal === 'SIGCONT' || signal === 'SIGINT') {
		runScript('signal-process.ps1', [
			'-Signal',
			signal,
			'-ProcessId',
			String(pid),
		]);
		return;
	}
	killTreeOnWindows(pid);
}

interface WindowsProcessFilter {
	executablePath?: string;
	commandLineMarker?: string;
	envMarker?: string;
}

/** Pids of the Windows processes with the file name `name` that pass `filter`. */
export function findWindowsProcesses(
	name: string,
	{ executablePath, commandLineMarker, envMarker }: WindowsProcessFilter,
): number[] {
	const args = ['-Name', name];
	if (executablePath !== undefined)
		args.push('-ExecutablePath', executablePath);
	if (commandLineMarker !== undefined) {
		args.push('-CommandLineMarker', commandLineMarker);
	}
	if (envMarker !== undefined) args.push('-EnvMarker', envMarker);
	return runScript('find-processes.ps1', args)
		.split(/\r?\n/)
		.filter(line => line !== '')
		.map(Number);
}
