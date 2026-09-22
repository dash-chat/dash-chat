/**
 * Which build each device currently has installed, so onPrepare installs an
 * app archive once per build instead of once per run. turbo (see
 * turbo-build.ts) guarantees the archive on disk matches the sources; this
 * records which device got which archive bytes. Android verifies directly
 * against the device (md5 of the installed base.apk); iOS can't checksum an
 * installed app, so this stamp is the record.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

// Not under .dbs/e2e — onPrepare wipes that dir every run and the record
// must survive across runs.
const STAMP_FILE = path.join(ROOT, '.dbs', 'e2e-device-installs.json');

/** Hex digest of a file's content; `algo` defaults to sha256 (md5 matches the
 *  on-device `md5sum` used to compare an installed APK). */
export function hashFile(file: string, algo = 'sha256'): string {
	return createHash(algo).update(readFileSync(file)).digest('hex');
}

/** Device udid -> what was last installed on it: the archive's sha256, plus an
 *  optional `marker` the caller reads back off the device (iOS passes the
 *  bundle container path, which a new install changes). The marker is what
 *  catches an app someone else installed over ours — a TestFlight or release
 *  build whose bytes we never saw, which the archive hash alone cannot rule
 *  out. */
type InstallStamp = { archive: string; marker?: string };
type InstallStamps = Record<string, InstallStamp>;

function readStamps(): InstallStamps {
	try {
		const parsed: unknown = JSON.parse(readFileSync(STAMP_FILE, 'utf8'));
		if (typeof parsed !== 'object' || parsed === null) return {};
		return Object.fromEntries(
			Object.entries(parsed as Record<string, unknown>).flatMap(
				([udid, stamp]) =>
					typeof stamp === 'object' && stamp !== null
						? [[udid, stamp as InstallStamp]]
						: [],
			),
		);
	} catch {
		return {};
	}
}

/** Whether `udid` already has this exact `archive` installed (per the stamp).
 *  `marker` is the caller's current reading of what is installed there now; it
 *  has to match the one recorded alongside the archive. */
export function deviceHasBuild(
	udid: string,
	archive: string,
	marker?: string,
): boolean {
	const stamp = readStamps()[udid];
	if (stamp === undefined || !existsSync(archive)) return false;
	return stamp.archive === hashFile(archive) && stamp.marker === marker;
}

/** Record that `archive` was installed on `udid`, with the `marker` read back
 *  off the device once it was. */
export function recordInstalled(
	udid: string,
	archive: string,
	marker?: string,
): void {
	const stamps: InstallStamps = {
		...readStamps(),
		[udid]: { archive: hashFile(archive), marker },
	};
	mkdirSync(path.dirname(STAMP_FILE), { recursive: true });
	writeFileSync(STAMP_FILE, JSON.stringify(stamps, null, '\t'));
}
