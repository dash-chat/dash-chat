import { readFileSync } from 'node:fs';

/** In the screenshots directory, the names of every scene the app has: what
 * tells a run that took them all from one that took some. */
export const SCENE_NAMES_FILE = 'scenes.json';

/** The key the app renders a source string under: i18next keeps each plural
 *  form of a message under its own suffixed key. */
export function renderedKey(sourceKey: string): string {
	return sourceKey.replace(/_(zero|one|two|few|many|other)$/, '');
}

/** Every key the app can render, from the source strings file. */
export function renderedSourceKeys(sourceStrings: string): string[] {
	const source = JSON.parse(readFileSync(sourceStrings, 'utf8')) as Record<
		string,
		string
	>;
	return [...new Set(Object.keys(source).map(renderedKey))];
}
