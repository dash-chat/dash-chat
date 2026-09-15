/**
 * Claims on what only one e2e run at a time may use — a phone, the host's
 * Wi-Fi card, a checkout — across every run on a machine, so that two
 * `just e2e` commands coexist: a run waits for what another holds instead
 * of taking it from under it. A claim is a file named after the thing,
 * holding the claiming process's pid, created exclusively so two runs can't
 * both win it; a claim whose process is gone is stale and taken over. The
 * files live in /tmp rather than os.tmpdir(): `nix develop` gives every
 * shell its own TMPDIR, and a claim has to be seen from every shell.
 */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const CLAIMS_DIR = '/tmp/dash-chat-e2e/claims';
const POLL_MS = 5_000;
const LOG_EVERY_MS = 60_000;

function claimPath(id: string): string {
	return path.join(CLAIMS_DIR, id);
}

function isAlive(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return true;
	} catch (err) {
		// Another user's process answers EPERM, and is just as alive.
		return (err as NodeJS.ErrnoException).code !== 'ESRCH';
	}
}

/** The pid holding `id`, or null when it is unclaimed or its claimant is gone. */
function holder(id: string): number | null {
	let text: string;
	try {
		text = readFileSync(claimPath(id), 'utf8');
	} catch {
		return null;
	}
	const pid = Number(text.trim());
	if (Number.isNaN(pid) || !isAlive(pid)) return null;
	return pid;
}

export function isFree(id: string): boolean {
	const pid = holder(id);
	return pid === null || pid === process.pid;
}

/** `ids` for an error message, naming those another run holds. */
export function describeHeld(ids: string[]): string {
	const list = (some: string[]) => some.join(', ') || 'none';
	const held = ids.filter(id => !isFree(id));
	if (held.length === 0) return `connected: ${list(ids)}`;
	return `connected: ${list(ids)}; driven by another e2e run: ${list(held)}`;
}

/** Take `id` for this process if no live process holds it. Exclusive
 *  creation decides a race; a stale claim is removed first. */
function tryClaim(id: string): boolean {
	const pid = holder(id);
	if (pid === process.pid) return true;
	if (pid !== null) return false;
	mkdirSync(CLAIMS_DIR, { recursive: true });
	rmSync(claimPath(id), { force: true });
	try {
		writeFileSync(claimPath(id), String(process.pid), { flag: 'wx' });
		return true;
	} catch {
		return false;
	}
}

/** Give `id` back, if this process holds it. */
export function release(id: string): void {
	if (holder(id) !== process.pid) return;
	rmSync(claimPath(id), { force: true });
}

/** Say once a minute who `id` is being waited on; returns when it last did. */
function noteWaiting(id: string, lastLog: number): number {
	if (Date.now() - lastLog < LOG_EVERY_MS) return lastLog;
	console.log(
		`[claim] waiting for ${id}, held by e2e run pid ${String(holder(id))}`,
	);
	return Date.now();
}

function sleepSync(ms: number): void {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/** Claim `id` once whoever holds it is done, saying so while waiting. */
export async function claimWhenFree(id: string): Promise<void> {
	let lastLog = 0;
	while (!tryClaim(id)) {
		lastLog = noteWaiting(id, lastLog);
		await new Promise(resolve => setTimeout(resolve, POLL_MS));
	}
}

/** What a run needs from one pool: `needed` of `candidates`, first come. */
export interface Want {
	candidates: string[];
	needed: number;
}

/**
 * Claim every group's `needed` candidates, once all can be held at once: on
 * any shortfall what was taken is released before waiting, so two runs each
 * holding half of what the other needs can never deadlock. Synchronous, for
 * the harness's config load, where nothing else runs anyway. A group with
 * fewer candidates than needed throws rather than waits forever.
 */
export function claimAllWhenFreeSync(wants: Want[]): string[][] {
	for (const { candidates, needed } of wants) {
		if (candidates.length < needed) {
			throw new Error(`cannot claim ${needed} of ${describeHeld(candidates)}`);
		}
	}
	let lastLog = 0;
	for (;;) {
		const taken = wants.map(want => takeUpTo(want));
		if (taken.every((got, i) => got.length === wants[i].needed)) return taken;
		for (const got of taken) for (const id of got) release(id);
		const busy = wants.flatMap(w => w.candidates).find(id => !isFree(id));
		if (busy !== undefined) lastLog = noteWaiting(busy, lastLog);
		sleepSync(POLL_MS);
	}
}

function takeUpTo({ candidates, needed }: Want): string[] {
	const got: string[] = [];
	for (const id of candidates) {
		if (got.length === needed) break;
		if (tryClaim(id)) got.push(id);
	}
	return got;
}
