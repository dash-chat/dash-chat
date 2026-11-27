import type { Screenshot } from 'i18n-screenshots/screenshot';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCREENSHOTS_DIR = fileURLToPath(
	new URL('../../.dbs/translation-screenshots/', import.meta.url),
);
const SCENE_NAMES_FILE = path.join(SCREENSHOTS_DIR, 'scenes.json');

function imageFile(sceneName: string): string {
	return path.join(SCREENSHOTS_DIR, `${sceneName}.png`);
}

function keysFile(sceneName: string): string {
	return path.join(SCREENSHOTS_DIR, `${sceneName}.keys.json`);
}

/** Replace the saved screenshots with `screenshots`, noting every scene the
 * app has, so that a set missing some can be told from a whole one. */
export function saveScreenshots(
	sceneNames: string[],
	screenshots: Screenshot[],
): void {
	rmSync(SCREENSHOTS_DIR, { recursive: true, force: true });
	mkdirSync(SCREENSHOTS_DIR, { recursive: true });
	writeFileSync(SCENE_NAMES_FILE, JSON.stringify(sceneNames, null, '\t'));
	for (const screenshot of screenshots) {
		writeFileSync(imageFile(screenshot.name), screenshot.image);
		writeFileSync(
			keysFile(screenshot.name),
			JSON.stringify(screenshot.keys, null, '\t'),
		);
	}
}

export function readSavedScreenshots(): {
	sceneNames: string[];
	screenshots: Screenshot[];
} {
	if (!existsSync(SCENE_NAMES_FILE)) {
		throw new Error(
			`No screenshots in ${SCREENSHOTS_DIR}: run \`just i18n screenshots\` first`,
		);
	}
	const sceneNames = JSON.parse(
		readFileSync(SCENE_NAMES_FILE, 'utf8'),
	) as string[];
	const screenshots = sceneNames
		.filter(name => existsSync(keysFile(name)))
		.map(name => ({
			name,
			image: readFileSync(imageFile(name)),
			keys: JSON.parse(readFileSync(keysFile(name), 'utf8')) as string[],
		}));
	return { sceneNames, screenshots };
}
