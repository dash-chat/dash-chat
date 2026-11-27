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
import { type Page, chromium } from 'playwright-core';

import type { Config } from './config.ts';
// For what it declares on `window`.
import type {} from './scene.ts';
import { SCENE_NAMES_FILE, renderedSourceKeys } from './source-keys.ts';

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

function buildMarkedApp(config: Config, configDirectory: string): void {
	const result = spawnSync(config.buildCommand, {
		cwd: configDirectory,
		stdio: 'inherit',
		shell: true,
	});
	if (result.status !== 0) {
		throw new Error(`${config.buildCommand} exited ${result.status}`);
	}
}

/** Serve the built app as the single-page app it is. */
function serveBuiltApp(builtApp: string): Promise<Server> {
	const server = createServer((request, response) => {
		const pathname = decodeURIComponent(
			new URL(request.url ?? '/', 'http://localhost').pathname,
		);
		const file = path.join(builtApp, pathname);
		const isAsset = file.startsWith(builtApp) && path.extname(file) !== '';
		if (isAsset && !existsSync(file)) {
			response.writeHead(404).end();
			return;
		}
		const served = isAsset ? file : path.join(builtApp, 'index.html');
		response.writeHead(200, {
			'Content-Type':
				CONTENT_TYPES[path.extname(served)] ?? 'application/octet-stream',
		});
		response.end(readFileSync(served));
	});
	return new Promise(resolve => server.listen(0, () => resolve(server)));
}

async function waitForTheScenes(page: Page): Promise<void> {
	await page.waitForFunction(
		() => window.__translationScreenshots !== undefined,
	);
}

async function scenesOfTheApp(
	page: Page,
	origin: string,
): Promise<{ name: string; device?: string }[]> {
	await page.goto(origin);
	await waitForTheScenes(page);
	const scenes = await page.evaluate(
		() => window.__translationScreenshots?.scenes ?? [],
	);
	await page.close();
	return scenes;
}

async function capture(
	page: Page,
	origin: string,
	sceneName: string,
	config: Config,
): Promise<string[]> {
	const url = new URL(origin);
	url.searchParams.set('scene', sceneName);
	await page.goto(url.href);
	await waitForTheScenes(page);
	const keys = await page.evaluate(
		() => window.__translationScreenshots?.shown() ?? [],
	);
	if (
		config.unexpectedErrorKey !== undefined &&
		keys.includes(config.unexpectedErrorKey) &&
		!sceneName.startsWith('error-')
	) {
		throw new Error('the app reported an unexpected error');
	}
	await page.screenshot({
		path: path.join(config.screenshots, `${sceneName}.png`),
	});
	writeFileSync(
		path.join(config.screenshots, `${sceneName}.keys.json`),
		JSON.stringify(keys, null, '\t'),
	);
	console.log(`${sceneName}: ${keys.length} strings`);
	return keys;
}

/** Fail the run when a source string no scene shows is not listed as such, or
 *  a listed one is shown after all: the list is how a new string without a
 *  screenshot gets noticed. */
function checkEveryStringIsAccountedFor(
	shown: Set<string>,
	config: Config,
): void {
	const listed = Object.values(
		JSON.parse(readFileSync(config.stringsWithoutScreenshot, 'utf8')) as Record<
			string,
			string[]
		>,
	).flat();
	const sourceKeys = renderedSourceKeys(config.sourceStrings);
	const notShown = sourceKeys.filter(key => !shown.has(key));
	console.log(
		`${sourceKeys.length - notShown.length}/${sourceKeys.length} source strings have a screenshot`,
	);

	const unlisted = notShown.filter(key => !listed.includes(key));
	const stale = listed.filter(key => !notShown.includes(key));
	if (unlisted.length > 0) {
		console.error(
			`No scene shows these strings; add a scene, or list them in ${config.stringsWithoutScreenshot}: ${unlisted.join(', ')}`,
		);
	}
	if (stale.length > 0) {
		console.error(
			`Listed in ${config.stringsWithoutScreenshot} but shown by a scene or gone from the source strings; remove them: ${stale.join(', ')}`,
		);
	}
	if (unlisted.length > 0 || stale.length > 0) process.exitCode = 1;
}

/** Build the app and take a screenshot of each of its scenes, or of `only`
 * the named ones, saving next to each the keys of the strings it shows. */
export async function captureScenes(
	config: Config,
	configDirectory: string,
	only: string[] | undefined,
): Promise<void> {
	buildMarkedApp(config, configDirectory);
	rmSync(config.screenshots, { recursive: true, force: true });
	mkdirSync(config.screenshots, { recursive: true });

	const server = await serveBuiltApp(config.builtApp);
	const origin = `http://localhost:${(server.address() as AddressInfo).port}`;
	// CHROME_BIN where the machine's Chrome is not where Playwright looks for it.
	const browser = await chromium.launch(
		process.env.CHROME_BIN === undefined
			? { channel: 'chrome' }
			: { executablePath: process.env.CHROME_BIN },
	);
	try {
		const scenes = await scenesOfTheApp(await browser.newPage(), origin);
		writeFileSync(
			path.join(config.screenshots, SCENE_NAMES_FILE),
			JSON.stringify(
				scenes.map(scene => scene.name),
				null,
				'\t',
			),
		);
		const shown = new Set<string>();
		for (const scene of scenes) {
			if (only !== undefined && !only.includes(scene.name)) continue;
			// A context of its own, so that a scene starts from nothing an
			// earlier one stored, and has no history but its own to go back in.
			const context = await browser.newContext(
				config.devices[scene.device ?? config.defaultDevice],
			);
			const page = await context.newPage();
			const pageErrors: string[] = [];
			page.on('pageerror', error => pageErrors.push(String(error)));
			page.on('console', message => {
				if (message.type() === 'error') pageErrors.push(message.text());
			});
			let failed = false;
			// One scene that cannot be taken must not hide what the others show.
			try {
				const keys = await Promise.race([
					capture(page, origin, scene.name, config),
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
			await context.close();
		}
		if (only === undefined) checkEveryStringIsAccountedFor(shown, config);
	} finally {
		await browser.close();
		server.close();
	}
}
