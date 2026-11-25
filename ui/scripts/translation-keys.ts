import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const SCREENSHOTS_DIR = fileURLToPath(
	new URL('../../.dbs/translation-screenshots/', import.meta.url),
);

/** The key the app renders a source string under: i18next keeps each plural
 *  form of a message under its own suffixed key. */
export function renderedKey(sourceKey: string): string {
	return sourceKey.replace(/_(zero|one|two|few|many|other)$/, '');
}

/** Every key the app can render, from the English source strings. */
export function renderedSourceKeys(): string[] {
	const source = JSON.parse(
		readFileSync(new URL('../messages/en.json', import.meta.url), 'utf8'),
	) as Record<string, string>;
	return [...new Set(Object.keys(source).map(renderedKey))];
}
