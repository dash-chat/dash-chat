import { type SceneOutcome, captureScenes } from 'i18n-screenshots/capture';
import { stringsNotAccountedFor } from 'i18n-screenshots/coverage';
import { existsSync, readFileSync } from 'node:fs';
import { type Server, createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { devices } from 'playwright-core';

import { saveScreenshots } from './files.ts';
import { renderedSourceKeys } from './rendered-keys.ts';

const UI = fileURLToPath(new URL('../../ui/', import.meta.url));
const UI_BUILD = path.join(UI, '.build-stage');
const SOURCE_STRINGS = path.join(UI, 'messages', 'en.json');
const STRINGS_WITHOUT_SCREENSHOT = path.join(
	UI,
	'src',
	'lib',
	'scenes',
	'strings-without-screenshot.json',
);

const CONTENT_TYPES: Record<string, string> = {
	'.html': 'text/html',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.jpg': 'image/jpeg',
	'.woff2': 'font/woff2',
	'.wasm': 'application/wasm',
};

const DESKTOP = {
	viewport: { width: 1100, height: 750 },
	deviceScaleFactor: 1.5,
};

// Scaled so that no screenshot outgrows the 2000×2000 pixels Weblate accepts.
const DEVICES = {
	phone: { ...devices['Pixel 7'], deviceScaleFactor: 2 },
	iphone: { ...devices['iPhone 14'], deviceScaleFactor: 2 },
	desktop: {
		...DESKTOP,
		userAgent:
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
	},
	'linux-desktop': {
		...DESKTOP,
		userAgent:
			'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
	},
};

/** Serve the built preview as the single-page app it is. */
function servePreview(): Promise<Server> {
	const server = createServer((request, response) => {
		const pathname = decodeURIComponent(
			new URL(request.url ?? '/', 'http://localhost').pathname,
		);
		const file = path.join(UI_BUILD, pathname);
		const isAsset = file.startsWith(UI_BUILD) && path.extname(file) !== '';
		if (isAsset && !existsSync(file)) {
			response.writeHead(404).end();
			return;
		}
		const served = isAsset ? file : path.join(UI_BUILD, 'index.html');
		response.writeHead(200, {
			'Content-Type':
				CONTENT_TYPES[path.extname(served)] ?? 'application/octet-stream',
		});
		response.end(readFileSync(served));
	});
	return new Promise(resolve => server.listen(0, () => resolve(server)));
}

// SCENES=a,b takes only those, to work on them without the rest.
const named = (process.env.SCENES ?? '')
	.split(',')
	.map(name => name.trim())
	.filter(name => name !== '');
const only = named.length > 0 ? named : undefined;

function report({ name, screenshot, error, pageErrors }: SceneOutcome): void {
	if (screenshot === undefined) console.error(`${name}: ${error}`);
	else console.log(`${name}: ${screenshot.keys.length} strings`);
	if (screenshot === undefined || only !== undefined) {
		for (const pageError of pageErrors) {
			console.error(`  ${pageError.split('\n')[0]}`);
		}
	}
}

/** Fail the run when a source string no scene shows is not listed as such, or
 *  a listed one is shown after all. */
function checkEveryStringIsAccountedFor(shownKeys: Set<string>): void {
	const sourceKeys = renderedSourceKeys(
		JSON.parse(readFileSync(SOURCE_STRINGS, 'utf8')) as Record<string, string>,
	);
	const listed = JSON.parse(
		readFileSync(STRINGS_WITHOUT_SCREENSHOT, 'utf8'),
	) as string[];
	const shown = sourceKeys.filter(key => shownKeys.has(key));
	console.log(
		`${shown.length}/${sourceKeys.length} source strings have a screenshot`,
	);

	const { unlisted, stale } = stringsNotAccountedFor(
		shownKeys,
		sourceKeys,
		listed,
	);
	if (unlisted.length > 0) {
		console.error(
			`No scene shows these strings; add a scene, or list them in ${STRINGS_WITHOUT_SCREENSHOT}: ${unlisted.join(', ')}`,
		);
	}
	if (stale.length > 0) {
		console.error(
			`Listed in ${STRINGS_WITHOUT_SCREENSHOT} but shown by a scene or gone from en.json; remove them: ${stale.join(', ')}`,
		);
	}
	if (unlisted.length > 0 || stale.length > 0) process.exitCode = 1;
}

const server = await servePreview();
try {
	const { sceneNames, outcomes } = await captureScenes({
		appUrl: `http://localhost:${(server.address() as AddressInfo).port}`,
		devices: DEVICES,
		defaultDevice: 'phone',
		unexpectedErrorKey: 'errorUnexpected',
		// CHROME_BIN where the machine's Chrome is not where Playwright looks for it.
		chrome: process.env.CHROME_BIN,
		only,
		onOutcome: report,
	});
	const screenshots = outcomes.flatMap(({ screenshot }) =>
		screenshot === undefined ? [] : [screenshot],
	);
	saveScreenshots(sceneNames, screenshots);
	if (screenshots.length < outcomes.length) process.exitCode = 1;
	if (only === undefined) {
		checkEveryStringIsAccountedFor(
			new Set(screenshots.flatMap(screenshot => screenshot.keys)),
		);
	}
} finally {
	server.close();
}
