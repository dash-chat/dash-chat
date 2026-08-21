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

/** Device udid -> sha256 of the app archive last installed on it. */
type InstallStamps = Record<string, string>;

function readStamps(): InstallStamps {
	try {
		return JSON.parse(readFileSync(STAMP_FILE, 'utf8')) as InstallStamps;
	} catch {
		return {};
	}
}

/** Whether `udid` already has this exact `archive` installed (per the stamp). */
export function deviceHasBuild(udid: string, archive: string): boolean {
	return existsSync(archive) && readStamps()[udid] === hashFile(archive);
}

/** Record that `archive` was installed on `udid`. */
export function recordInstalled(udid: string, archive: string): void {
	const stamps = { ...readStamps(), [udid]: hashFile(archive) };
	mkdirSync(path.dirname(STAMP_FILE), { recursive: true });
	writeFileSync(STAMP_FILE, JSON.stringify(stamps, null, '\t'));
}
