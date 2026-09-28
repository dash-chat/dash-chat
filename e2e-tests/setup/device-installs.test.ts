import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import { deviceHasBuild, recordInstalled } from './device-installs.ts';

// Set before the module under test reads it, so nothing here touches the
// checkout's own stamp file.
const STAMP_FILE = path.join(
	mkdtempSync(path.join(tmpdir(), 'device-installs-')),
	'stamps.json',
);
process.env.E2E_DEVICE_INSTALLS_FILE = STAMP_FILE;

/** An archive to hash, and a udid no real device has. */
const archive = path.join(
	mkdtempSync(path.join(tmpdir(), 'device-installs-')),
	'app.ipa',
);
writeFileSync(archive, 'the bytes we installed');

let counter = 0;
function freshUdid(): string {
	counter += 1;
	return `test-${process.pid}-${counter}`;
}

test('a device keeps the build it was given', () => {
	const udid = freshUdid();
	recordInstalled(udid, archive, 'bundle-a');
	assert.equal(
		deviceHasBuild(udid, archive, () => 'bundle-a'),
		true,
	);
});

test('a build someone else installed over ours is reinstalled', () => {
	const udid = freshUdid();
	recordInstalled(udid, archive, 'bundle-a');
	// Another install moved the app to a container of its own.
	assert.equal(
		deviceHasBuild(udid, archive, () => 'bundle-b'),
		false,
	);
});

test('a marker that could not be read is reinstalled', () => {
	const udid = freshUdid();
	recordInstalled(udid, archive, 'bundle-a');
	// The device could not be asked. Two unknowns must not match.
	assert.equal(
		deviceHasBuild(udid, archive, () => undefined),
		false,
	);
});

test('an unreadable marker records nothing to match next time', () => {
	const udid = freshUdid();
	recordInstalled(udid, archive, undefined);
	assert.equal(
		deviceHasBuild(udid, archive, () => 'bundle-a'),
		false,
	);
});

test('a stamp from before the marker existed is reinstalled', () => {
	const udid = freshUdid();
	recordInstalled(udid, archive, 'bundle-a');
	const stamps: Record<string, unknown> = JSON.parse(
		readFileSync(STAMP_FILE, 'utf8'),
	);
	// The old format: the archive hash alone, as a bare string.
	stamps[udid] = 'a'.repeat(64);
	writeFileSync(STAMP_FILE, JSON.stringify(stamps));
	assert.equal(
		deviceHasBuild(udid, archive, () => 'bundle-a'),
		false,
	);
});

test('a device nobody recorded is reinstalled', () => {
	assert.equal(
		deviceHasBuild(freshUdid(), archive, () => 'bundle-a'),
		false,
	);
});
