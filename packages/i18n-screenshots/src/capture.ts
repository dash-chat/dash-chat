import {
	type Browser,
	type BrowserContextOptions,
	type Page,
	chromium,
} from 'playwright-core';

// For what it declares on `window`.
import type {} from './scene.ts';

/** The picture of one scene and what it shows. */
export interface Screenshot {
	/** The name of the scene it is of. */
	name: string;
	/** The picture, as PNG. */
	image: Uint8Array;
	/** The keys of the strings it shows. */
	keys: string[];
}

export interface CaptureOptions {
	/** Where the app is served, built with every message carrying its key. */
	appUrl: string;
	/** What a scene can be rendered as, by the name scenes call it. */
	devices: Record<string, BrowserContextOptions>;
	/** The device of a scene that names none. */
	defaultDevice: string;
	/** The Chrome to drive, where it is not where Playwright looks for it. */
	chrome?: string;
	/** The names of the only scenes to take, to work on them without the
	 * rest. */
	only?: string[];
	/** Told of each scene as it is done with, to follow a long run. */
	onOutcome?: (outcome: SceneOutcome) => void;
}

export interface SceneOutcome {
	name: string;
	/** Absent when the scene could not be taken, which `error` explains. */
	screenshot?: Screenshot;
	error?: string;
	/** What the page reported as errors while the scene played. */
	pageErrors: string[];
}

interface SceneToCapture {
	name: string;
	device?: string;
}

async function waitForTheScenes(page: Page): Promise<void> {
	await page.waitForFunction(() => window.__i18nScreenshots !== undefined);
}

async function scenesOfTheApp(
	page: Page,
	appUrl: string,
): Promise<SceneToCapture[]> {
	await page.goto(appUrl);
	await waitForTheScenes(page);
	const scenes = await page.evaluate(
		() => window.__i18nScreenshots?.scenes ?? [],
	);
	await page.close();
	return scenes;
}

function keysShown(page: Page): Promise<string[]> {
	return page.evaluate(() => window.__i18nScreenshots?.shown() ?? []);
}

/** The picture of the page with the keys it shows. The page may still change
 * between the two being taken, so the picture counts only when the keys were
 * the same right before and right after it. */
async function pictureWithItsKeys(
	page: Page,
): Promise<{ image: Uint8Array; keys: string[] }> {
	let keys = await keysShown(page);
	for (;;) {
		const image = await page.screenshot();
		const keysAfter = await keysShown(page);
		if (keysAfter.join() === keys.join()) return { image, keys };
		keys = keysAfter;
	}
}

async function capture(
	page: Page,
	sceneName: string,
	options: CaptureOptions,
): Promise<Screenshot> {
	const url = new URL(options.appUrl);
	url.searchParams.set('scene', sceneName);
	await page.goto(url.href);
	await waitForTheScenes(page);
	return { name: sceneName, ...(await pictureWithItsKeys(page)) };
}

const NEVER_SETTLED_AFTER_MS = 30_000;

/** What `taking` resolves to, unless it takes too long to be going anywhere. */
async function unlessItNeverSettles<T>(taking: Promise<T>): Promise<T> {
	let givingUp: NodeJS.Timeout | undefined;
	try {
		return await Promise.race([
			taking,
			new Promise<never>((_, reject) => {
				givingUp = setTimeout(
					() => reject(new Error('never settled')),
					NEVER_SETTLED_AFTER_MS,
				);
			}),
		]);
	} finally {
		clearTimeout(givingUp);
	}
}

/** Take one scene. One that cannot be taken says so in its outcome, not by
 * rejecting: it must not hide what the others show. */
async function captureOnItsDevice(
	browser: Browser,
	scene: SceneToCapture,
	options: CaptureOptions,
): Promise<SceneOutcome> {
	// A context of its own, so that a scene starts from nothing an earlier
	// one stored, and has no history but its own to go back in.
	const context = await browser.newContext(
		options.devices[scene.device ?? options.defaultDevice],
	);
	const page = await context.newPage();
	const outcome: SceneOutcome = { name: scene.name, pageErrors: [] };
	page.on('pageerror', error => outcome.pageErrors.push(String(error)));
	page.on('console', message => {
		if (message.type() === 'error') outcome.pageErrors.push(message.text());
	});
	try {
		outcome.screenshot = await unlessItNeverSettles(
			capture(page, scene.name, options),
		);
	} catch (error) {
		outcome.error = String(error).split('\n')[0];
	}
	await context.close();
	return outcome;
}

/** Before any scene is taken: found halfway through, a scene without a
 * device would throw away the ones taken before it. */
function refuseUnknownDevices(
	scenes: SceneToCapture[],
	options: CaptureOptions,
): void {
	for (const scene of scenes) {
		const deviceName = scene.device ?? options.defaultDevice;
		if (options.devices[deviceName] === undefined) {
			throw new Error(
				`${scene.name} is to be rendered as "${deviceName}", which there is no device for`,
			);
		}
	}
}

function refuseUnknownScenes(only: string[], scenes: SceneToCapture[]): void {
	const unknown = only.filter(
		name => !scenes.some(scene => scene.name === name),
	);
	if (unknown.length > 0) {
		throw new Error(`The app has no scene named ${unknown.join(', ')}`);
	}
}

/** Take a screenshot of each scene of the app served at `appUrl`, with the
 * keys of the strings each one shows. */
export async function captureScenes(
	options: CaptureOptions,
): Promise<SceneOutcome[]> {
	const { only } = options;
	const browser = await chromium.launch(
		options.chrome === undefined
			? { channel: 'chrome' }
			: { executablePath: options.chrome },
	);
	try {
		const scenes = await scenesOfTheApp(
			await browser.newPage(),
			options.appUrl,
		);
		if (only !== undefined) refuseUnknownScenes(only, scenes);
		refuseUnknownDevices(scenes, options);
		const outcomes: SceneOutcome[] = [];
		for (const scene of scenes.filter(
			scene => only === undefined || only.includes(scene.name),
		)) {
			const outcome = await captureOnItsDevice(browser, scene, options);
			options.onOutcome?.(outcome);
			outcomes.push(outcome);
		}
		return outcomes;
	} finally {
		await browser.close();
	}
}
