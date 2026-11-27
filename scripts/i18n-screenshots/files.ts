import type { Screenshot } from 'i18n-screenshots/capture';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	renameSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// What is synced to Weblate, which deletes there whatever is missing here:
// so it only ever holds one screenshot of every scene, or does not exist.
const SCREENSHOTS_OF_EVERY_SCENE = fileURLToPath(
	new URL('../../.dbs/translation-screenshots/', import.meta.url),
);
const SCREENSHOTS_OF_A_RUN_MISSING_SCENES = fileURLToPath(
	new URL('../../.dbs/translation-screenshots-partial/', import.meta.url),
);

const KEYS_SUFFIX = '.keys.json';

function write(screenshots: Screenshot[], directory: string): void {
	rmSync(directory, { recursive: true, force: true });
	mkdirSync(directory, { recursive: true });
	for (const { name, image, keys } of screenshots) {
		writeFileSync(path.join(directory, `${name}.png`), image);
		writeFileSync(
			path.join(directory, `${name}${KEYS_SUFFIX}`),
			JSON.stringify(keys, null, '\t'),
		);
	}
}

/** Save one screenshot of every scene as what is synced, resolving to where.
 * Written aside and moved into place, so that being interrupted leaves the
 * set as it was and never half of it. */
export function saveScreenshotsOfEveryScene(screenshots: Screenshot[]): string {
	const beingWritten = `${path.resolve(SCREENSHOTS_OF_EVERY_SCENE)}.writing`;
	write(screenshots, beingWritten);
	rmSync(SCREENSHOTS_OF_EVERY_SCENE, { recursive: true, force: true });
	renameSync(beingWritten, SCREENSHOTS_OF_EVERY_SCENE);
	return SCREENSHOTS_OF_EVERY_SCENE;
}

/** Save what a run that did not take every scene did take, resolving to
 * where. With `whatIsSyncedIsOutdated`, a run that set out to take them all
 * and could not, what was to be synced goes too: it is no longer of the app
 * as it is. */
export function saveScreenshotsOfSomeScenes(
	screenshots: Screenshot[],
	whatIsSyncedIsOutdated: boolean,
): string {
	write(screenshots, SCREENSHOTS_OF_A_RUN_MISSING_SCENES);
	if (whatIsSyncedIsOutdated) {
		rmSync(SCREENSHOTS_OF_EVERY_SCENE, { recursive: true, force: true });
	}
	return SCREENSHOTS_OF_A_RUN_MISSING_SCENES;
}

export function readScreenshotsOfEveryScene(): Screenshot[] {
	if (!existsSync(SCREENSHOTS_OF_EVERY_SCENE)) {
		throw new Error(
			`No screenshots of every scene in ${SCREENSHOTS_OF_EVERY_SCENE}: run \`just i18n screenshots\` until it passes`,
		);
	}
	return readdirSync(SCREENSHOTS_OF_EVERY_SCENE)
		.filter(file => file.endsWith(KEYS_SUFFIX))
		.map(file => file.slice(0, -KEYS_SUFFIX.length))
		.map(name => ({
			name,
			image: readFileSync(path.join(SCREENSHOTS_OF_EVERY_SCENE, `${name}.png`)),
			keys: JSON.parse(
				readFileSync(
					path.join(SCREENSHOTS_OF_EVERY_SCENE, `${name}${KEYS_SUFFIX}`),
					'utf8',
				),
			) as string[],
		}));
}
