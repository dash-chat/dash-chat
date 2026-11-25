import { spawnSync } from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { type Server, createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Page, chromium, devices } from 'playwright-core';

import type {
	SceneStepToCapture,
	SceneToCapture,
} from '../tests/setup-utils.ts';
import { SCREENSHOTS_DIR, renderedSourceKeys } from './translation-keys.ts';

const UI = fileURLToPath(new URL('../', import.meta.url));
const UI_BUILD = path.join(UI, '.build-stage');
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

/** Build the browser preview of the UI with every message carrying its key.
 *  `vite build` alone stops at the staging directory, so the build the app
 *  embeds is left as it was. */
function buildMarkedPreview(): void {
	const result = spawnSync('pnpm', ['exec', 'vite', 'build'], {
		cwd: UI,
		stdio: 'inherit',
		env: { ...process.env, VITE_TRANSLATION_KEY_MARKERS: 'true' },
	});
	if (result.status !== 0)
		throw new Error(`vite build exited ${result.status}`);
}

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

/** Comfortably past the threshold of the app's long-press. */
const LONG_PRESS_MS = 700;

// Scaled so that no screenshot outgrows the 2000×2000 pixels Weblate accepts.
const DEVICES = {
	phone: { ...devices['Pixel 7'], deviceScaleFactor: 2 },
	iphone: { ...devices['iPhone 14'], deviceScaleFactor: 2 },
	desktop: {
		viewport: { width: 1100, height: 750 },
		deviceScaleFactor: 1.5,
		userAgent:
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
	},
	'linux-desktop': {
		viewport: { width: 1100, height: 750 },
		deviceScaleFactor: 1.5,
		userAgent:
			'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
	},
};

function touchAtCentreOf(
	box: { x: number; y: number; width: number; height: number } | null,
) {
	if (box === null) throw new Error('nothing to long-press');
	const touch = {
		identifier: 1,
		clientX: box.x + box.width / 2,
		clientY: box.y + box.height / 2,
	};
	return { touches: [touch], targetTouches: [touch], changedTouches: [touch] };
}

async function waitForApp(page: Page): Promise<void> {
	await page.waitForFunction(
		() =>
			window.__test !== undefined &&
			document.fonts.status === 'loaded' &&
			window.__test.visibleTranslationKeys().length > 0,
	);
}

/** The message keys on screen once they have stopped changing: data arrives
 *  and dialogs mount a few frames after what triggered them. */
async function settledTranslationKeys(page: Page): Promise<string[]> {
	const readAfterTwoFrames = async (): Promise<string[]> => {
		try {
			return await page.evaluate(
				() =>
					new Promise<string[]>(resolve =>
						requestAnimationFrame(() =>
							requestAnimationFrame(() =>
								// Rejecting from inside the frame would leave this pending.
								resolve(window.__test?.visibleTranslationKeys() ?? []),
							),
						),
					),
			);
		} catch {
			// A step can make the app load another document; read that one.
			await waitForApp(page);
			return [];
		}
	};
	let keys = await readAfterTwoFrames();
	for (;;) {
		const next = await readAfterTwoFrames();
		if (next.join() === keys.join()) return keys;
		keys = next;
	}
}

async function perform(
	page: Page,
	step: SceneStepToCapture,
	hasTouch: boolean,
): Promise<void> {
	if ('run' in step) {
		await page.evaluate(index => window.__test.runSceneStep(index), step.run);
	} else if ('type' in step) {
		const field = `[data-testid="${step.into}"]`;
		await page
			.locator(`${field}:is(input, textarea), ${field} :is(input, textarea)`)
			.first()
			.fill(step.type);
	} else if ('hover' in step) {
		await page.getByTestId(step.hover).first().hover();
	} else if ('hold' in step) {
		const held = page.getByTestId(step.hold).first();
		await held.dispatchEvent(
			'touchstart',
			touchAtCentreOf(await held.boundingBox()),
		);
		await page.waitForTimeout(LONG_PRESS_MS);
		await held.dispatchEvent('touchend', { touches: [], changedTouches: [] });
	} else {
		const target = page.locator(`[data-testid="${step.tap}"]:visible`).first();
		// Only the phones have a touchscreen to tap.
		if (hasTouch) await target.tap();
		else await target.click();
	}
}

async function capture(
	page: Page,
	origin: string,
	scene: SceneToCapture,
): Promise<string[]> {
	const url = new URL(scene.path, origin);
	url.searchParams.set('scene', scene.name);
	await page.goto(url.href);
	await waitForApp(page);
	// An entry of its own to go back to: a scene that goes back in history
	// would otherwise leave the app for the blank page the tab opened on.
	await page.evaluate(() =>
		history.pushState(history.state, '', location.href),
	);
	await page.addStyleTag({
		content:
			'*, *::before, *::after { transition: none !important; animation: none !important; }',
	});
	for (const step of scene.steps) {
		await perform(
			page,
			step,
			scene.device === undefined || scene.device === 'iphone',
		);
		// The next step may be on what this one brings up.
		await settledTranslationKeys(page);
	}
	const keys = [...(await settledTranslationKeys(page)), ...scene.drawn];
	// The app's catch-all error means the scene broke, unless the scene is of it.
	if (keys.includes('errorUnexpected') && !scene.name.startsWith('error-')) {
		throw new Error('the app reported an unexpected error');
	}
	await page.screenshot({
		path: path.join(SCREENSHOTS_DIR, `${scene.name}.png`),
	});
	writeFileSync(
		path.join(SCREENSHOTS_DIR, `${scene.name}.keys.json`),
		JSON.stringify(keys, null, '\t'),
	);
	console.log(`${scene.name}: ${keys.length} strings`);
	return keys;
}

/** Fail the run when a source string no scene shows is not listed as such, or
 *  a listed one is shown after all: the list is how a new string without a
 *  screenshot gets noticed. */
function checkEveryStringIsAccountedFor(shown: Set<string>): void {
	const listed = Object.values(
		JSON.parse(readFileSync(STRINGS_WITHOUT_SCREENSHOT, 'utf8')) as Record<
			string,
			string[]
		>,
	).flat();
	const sourceKeys = renderedSourceKeys();
	const notShown = sourceKeys.filter(key => !shown.has(key));
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

async function scenesToCapture(
	page: Page,
	origin: string,
): Promise<SceneToCapture[]> {
	await page.goto(origin);
	await waitForApp(page);
	const scenes = await page.evaluate(() => window.__test.scenes);
	await page.close();
	return scenes;
}

buildMarkedPreview();
rmSync(SCREENSHOTS_DIR, { recursive: true, force: true });
mkdirSync(SCREENSHOTS_DIR, { recursive: true });

const server = await servePreview();
const origin = `http://localhost:${(server.address() as AddressInfo).port}`;
// CHROME_BIN where the machine's Chrome is not where Playwright looks for it.
const browser = await chromium.launch(
	process.env.CHROME_BIN === undefined
		? { channel: 'chrome' }
		: { executablePath: process.env.CHROME_BIN },
);
try {
	const contexts = {
		phone: await browser.newContext(DEVICES.phone),
		iphone: await browser.newContext(DEVICES.iphone),
		desktop: await browser.newContext(DEVICES.desktop),
		'linux-desktop': await browser.newContext(DEVICES['linux-desktop']),
	};
	const scenes = await scenesToCapture(await contexts.phone.newPage(), origin);
	// SCENES=a,b takes only those, to work on them without the rest.
	const only = process.env.SCENES?.split(',');
	const shown = new Set<string>();
	for (const scene of scenes) {
		if (only !== undefined && !only.includes(scene.name)) continue;
		// A page of its own, so that a scene going back in history has nowhere
		// to go but stay: on a shared page it would land in the scene before.
		const page = await contexts[scene.device ?? 'phone'].newPage();
		// Everything here is local and takes moments; a scene that cannot be
		// taken should say so quickly.
		page.setDefaultTimeout(10_000);
		const pageErrors: string[] = [];
		page.on('pageerror', error => pageErrors.push(String(error)));
		page.on('console', message => {
			if (message.type() === 'error') pageErrors.push(message.text());
		});
		let failed = false;
		// One scene that cannot be taken must not hide what the others show.
		try {
			const keys = await Promise.race([
				capture(page, origin, scene),
				new Promise<never>((_, reject) =>
					setTimeout(() => reject(new Error('never settled')), 30_000),
				),
			]);
			for (const key of keys) shown.add(key);
		} catch (error) {
			console.error(`${scene.name}: ${String(error).split('\n')[0]}`);
			failed = true;
			process.exitCode = 1;
		}
		if (failed || only !== undefined) {
			for (const pageError of pageErrors) {
				console.error(`  ${pageError.split('\n')[0]}`);
			}
		}
		await page.close();
	}
	if (only === undefined) checkEveryStringIsAccountedFor(shown);
} finally {
	await browser.close();
	server.close();
}
