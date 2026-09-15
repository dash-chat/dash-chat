/**
 * Claims on the physical things only one run at a time can drive — a phone,
 * the host's Wi-Fi card — across the runs of one machine, where two checkouts
 * launching together would otherwise both take the first phone `adb devices`
 * or `idevice_id` lists, or both move the card. A claim is a file named after
 * the device's id, holding the claiming process's pid; a claim whose process
 * is gone is stale and free to take. They live in the temp dir: a claim
 * outlives no boot, and neither does the pid it names.
 */
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const LOCK_DIR = path.join(os.tmpdir(), 'dash-chat-e2e', 'devices');

function lockPath(udid: string): string {
	return path.join(LOCK_DIR, udid);
}

/** The pid holding `udid`, or null when it is unclaimed or its claimant is gone. */
function holder(udid: string): number | null {
	if (!existsSync(lockPath(udid))) return null;
	const pid = Number(readFileSync(lockPath(udid), 'utf8').trim());
	if (Number.isNaN(pid)) return null;
	try {
		process.kill(pid, 0);
		return pid;
	} catch {
		return null;
	}
}

export function isDeviceFree(udid: string): boolean {
	const pid = holder(udid);
	return pid === null || pid === process.pid;
}

/** Claim `udid` for this process. Throws naming the run that holds it. */
export function claimDevice(udid: string): void {
	const pid = holder(udid);
	if (pid !== null && pid !== process.pid) {
		throw new Error(
			`device ${udid} is driven by another e2e run (pid ${pid}); wait for it, ` +
				'pick another device, or remove its claim under ' +
				`${LOCK_DIR} if that run is gone`,
		);
	}
	mkdirSync(LOCK_DIR, { recursive: true });
	writeFileSync(lockPath(udid), String(process.pid));
}

const CLAIM_POLL_MS = 5_000;
const CLAIM_LOG_EVERY_MS = 60_000;

/** Claim `udid` for this process once whoever holds it is done with it,
 *  saying so while waiting. The caller's own timeout bounds the wait. */
export async function claimDeviceWhenFree(udid: string): Promise<void> {
	let lastLog = 0;
	for (;;) {
		const pid = holder(udid);
		if (pid === null || pid === process.pid) break;
		if (Date.now() - lastLog >= CLAIM_LOG_EVERY_MS) {
			console.log(`[claim] waiting for ${udid}, driven by e2e run pid ${pid}`);
			lastLog = Date.now();
		}
		await new Promise(resolve => setTimeout(resolve, CLAIM_POLL_MS));
	}
	claimDevice(udid);
}

/** Give `udid` back, if this process holds it. */
export function releaseDevice(udid: string): void {
	if (holder(udid) !== process.pid) return;
	rmSync(lockPath(udid), { force: true });
}
