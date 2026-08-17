/**
 * Copy the freshest .ipa from the tauri ios build dir to the fixed path the
 * e2e sessions install from. Runs as the last step of the `e2e:build:ios`
 * task so turbo snapshots the final artifact: the build dir's layout is
 * rearranged by tauri between versions (the .ipa is exported to the build
 * dir, then moved into an arch subdir), so newest-mtime wins.
 */
import { execSync } from 'node:child_process';
import {
	constants,
	copyFileSync,
	existsSync,
	mkdirSync,
	statSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const IOS_BUILD_DIR = path.join(ROOT, 'src-tauri/gen/apple/build');
const SESSION_IPA = path.join(ROOT, 'e2e-tests/.appium/dash-chat-e2e.ipa');

const out = existsSync(IOS_BUILD_DIR)
	? execSync(`find "${IOS_BUILD_DIR}" -maxdepth 3 -name '*.ipa'`, {
			encoding: 'utf8',
		}).trim()
	: '';
const ipas = out
	.split('\n')
	.filter(Boolean)
	.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
if (ipas.length === 0) {
	console.error(`No .ipa found under ${IOS_BUILD_DIR} after 'tauri ios build'`);
	process.exit(1);
}
mkdirSync(path.dirname(SESSION_IPA), { recursive: true });
// COPYFILE_FICLONE: APFS clones instead of duplicating the ~135MB payload.
copyFileSync(ipas[0], SESSION_IPA, constants.COPYFILE_FICLONE);
console.log(`[ios] exported ${ipas[0]} -> ${SESSION_IPA}`);
