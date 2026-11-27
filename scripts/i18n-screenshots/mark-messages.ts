import { markedSourceStrings } from 'i18n-screenshots/markers';
import { readFileSync, writeFileSync } from 'node:fs';

import { renderedKey } from './rendered-keys.ts';

const [sourceStrings, markedStrings] = process.argv.slice(2);

writeFileSync(
	markedStrings,
	JSON.stringify(
		markedSourceStrings(
			JSON.parse(readFileSync(sourceStrings, 'utf8')) as Record<string, string>,
			renderedKey,
		),
	),
);
