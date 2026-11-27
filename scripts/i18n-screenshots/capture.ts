import { syncScreenshotsToWeblate } from 'i18n-screenshots-weblate';
import {
	type SceneOutcome,
	type Screenshot,
	captureScenes,
} from 'i18n-screenshots/capture';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { devices } from 'playwright-core';

import { renderedKey, renderedSourceKeys } from './rendered-keys.ts';

const UI = fileURLToPath(new URL('../../ui/', import.meta.url));
const SCREENSHOTS = fileURLToPath(
	new URL('../../.dbs/translation-screenshots/', import.meta.url),
);
const SOURCE_STRINGS = path.join(UI, 'messages', 'en.json');
const STRINGS_WITHOUT_SCREENSHOT = path.join(
	UI,
	'src',
	'lib',
	'scenes',
	'strings-without-screenshot.json',
);

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

// SCENES=a,b takes only those, to work on them without the rest.
const named = (process.env.SCENES ?? '')
	.split(',')
	.map(name => name.trim())
	.filter(name => name !== '');
const only = named.length > 0 ? named : undefined;

function whyUnusable({
	name,
	screenshot,
	error,
}: SceneOutcome): string | undefined {
	if (screenshot === undefined) return error;
	// The app's catch-all error means the scene broke, unless the scene is of it.
	if (
		screenshot.keys.includes('errorUnexpected') &&
		!name.startsWith('error-')
	) {
		return 'the app reported an unexpected error';
	}
	return undefined;
}

function report(outcome: SceneOutcome): void {
	const { name, screenshot, pageErrors } = outcome;
	const failure = whyUnusable(outcome);
	if (failure !== undefined) console.error(`${name}: ${failure}`);
	else console.log(`${name}: ${screenshot?.keys.length} strings`);
	if (failure !== undefined || only !== undefined) {
		for (const pageError of pageErrors) {
			console.error(`  ${pageError.split('\n')[0]}`);
		}
	}
}

/** Fail the run when a source string no scene shows is not listed as such, or
 *  a listed one is shown after all: the list is how a new string without a
 *  screenshot gets noticed. */
function checkEveryStringIsAccountedFor(shownKeys: Set<string>): void {
	const sourceKeys = renderedSourceKeys(
		JSON.parse(readFileSync(SOURCE_STRINGS, 'utf8')) as Record<string, string>,
	);
	const listed = JSON.parse(
		readFileSync(STRINGS_WITHOUT_SCREENSHOT, 'utf8'),
	) as string[];
	const notShown = sourceKeys.filter(key => !shownKeys.has(key));
	console.log(
		`${sourceKeys.length - notShown.length}/${sourceKeys.length} source strings have a screenshot`,
	);

	const unlisted = notShown.filter(key => !listed.includes(key));
	const stale = listed.filter(key => !notShown.includes(key));
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

/** Save the screenshots, and the keys each shows, for someone to look at. */
function save(screenshots: Screenshot[]): void {
	rmSync(SCREENSHOTS, { recursive: true, force: true });
	mkdirSync(SCREENSHOTS, { recursive: true });
	for (const { name, image, keys } of screenshots) {
		writeFileSync(path.join(SCREENSHOTS, `${name}.png`), image);
		writeFileSync(
			path.join(SCREENSHOTS, `${name}.keys.json`),
			JSON.stringify(keys, null, '\t'),
		);
	}
}

const [appUrl, thenSync] = process.argv.slice(2);
if (appUrl === undefined || (thenSync !== undefined && thenSync !== 'sync')) {
	throw new Error(
		'Usage: capture.ts <the URL the marked preview is served at> [sync]; `just i18n screenshots` builds and serves it',
	);
}
const weblateToken = process.env.WEBLATE_TOKEN;
if (thenSync !== undefined) {
	// Before minutes are spent taking screenshots that could not be synced.
	if (!weblateToken) throw new Error('WEBLATE_TOKEN is not set');
	// Weblate would lose the screenshot of every scene left out.
	if (only !== undefined) throw new Error('SCENES cannot be synced');
}

/** Resolves once the preview answers: it is started alongside this. */
async function untilItIsServed(url: string): Promise<void> {
	for (;;) {
		const served = await fetch(url).then(
			response => response.ok,
			() => false,
		);
		if (served) return;
		await new Promise(resolve => setTimeout(resolve, 200));
	}
}

await untilItIsServed(appUrl);
const outcomes = await captureScenes({
	appUrl,
	devices: DEVICES,
	defaultDevice: 'phone',
	// CHROME_BIN where the machine's Chrome is not where Playwright looks for it.
	chrome: process.env.CHROME_BIN,
	only,
	onOutcome: report,
});
const screenshots = outcomes.flatMap(outcome =>
	outcome.screenshot === undefined || whyUnusable(outcome) !== undefined
		? []
		: [outcome.screenshot],
);
save(screenshots);
if (screenshots.length < outcomes.length) process.exitCode = 1;
if (only === undefined) {
	checkEveryStringIsAccountedFor(
		new Set(screenshots.flatMap(screenshot => screenshot.keys)),
	);
}

// Straight from what was just taken, and only when nothing above failed:
// Weblate deletes the screenshots it is not handed.
if (weblateToken && thenSync !== undefined && process.exitCode === undefined) {
	await syncScreenshotsToWeblate({
		screenshots,
		url: 'https://nuance.weblate.cloud',
		token: weblateToken,
		project: 'dash-chat',
		component: 'dash-chat-app',
		sourceLanguage: 'en',
		renderedKey,
		log: console.log,
	});
}
