#!/usr/bin/env node
import path from 'node:path';

import { loadConfig } from './config.ts';

const [command, configFile = 'translation-screenshots.config.ts'] =
	process.argv.slice(2);
const config = await loadConfig(configFile);

// Each command loads only what it needs: syncing needs no browser, and
// taking the screenshots no Weblate account.
if (command === 'capture') {
	const { captureScenes } = await import('./capture.ts');
	// SCENES=a,b takes only those, to work on them without the rest.
	await captureScenes(
		config,
		path.dirname(path.resolve(configFile)),
		process.env.SCENES?.split(','),
	);
} else if (command === 'sync-weblate') {
	const { syncScreenshotsToWeblate } = await import('./weblate.ts');
	await syncScreenshotsToWeblate(config);
} else {
	console.error(
		'Usage: translation-screenshots <capture | sync-weblate> [config file]',
	);
	process.exitCode = 1;
}
