import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { type Screenshot, WeblateClient } from 'weblate-client';

import type { Config } from './config.ts';
import { SCENE_NAMES_FILE, renderedKey } from './source-keys.ts';

const SOURCE_LANGUAGE = 'en';
// Marks the screenshots this owns, so the ones translators upload by hand
// are never replaced or deleted.
const MANAGED_NAME_PREFIX = 'scene: ';

interface LocalScreenshot {
	name: string;
	image: string;
	keys: string[];
}

/** A local screenshot and the Weblate strings it shows. */
interface ScreenshotToSync extends LocalScreenshot {
	unitIds: number[];
}

function readLocalScreenshots(screenshots: string): LocalScreenshot[] {
	return readdirSync(screenshots)
		.filter(file => file.endsWith('.keys.json'))
		.map(file => {
			const name = file.slice(0, -'.keys.json'.length);
			return {
				name: `${MANAGED_NAME_PREFIX}${name}`,
				image: path.join(screenshots, `${name}.png`),
				keys: JSON.parse(
					readFileSync(path.join(screenshots, file), 'utf8'),
				) as string[],
			};
		});
}

/** Throws unless `screenshots` holds one of every scene the app has: what
 * is missing would be deleted from Weblate as no longer existing. */
function refuseAPartialRun(screenshots: string): void {
	const sceneNamesFile = path.join(screenshots, SCENE_NAMES_FILE);
	if (!existsSync(sceneNamesFile)) {
		throw new Error(`No screenshots in ${screenshots}: take them first`);
	}
	const missing = (
		JSON.parse(readFileSync(sceneNamesFile, 'utf8')) as string[]
	).filter(name => !existsSync(path.join(screenshots, `${name}.keys.json`)));
	if (missing.length > 0) {
		throw new Error(
			`Not syncing: ${screenshots} has no screenshot of ${missing.length} scenes, which would be deleted from Weblate. Take them all first. Missing: ${missing.join(', ')}`,
		);
	}
}

/** Attach `screenshot` to exactly the units in `wantedUnitIds`. */
async function attachToUnits(
	weblate: WeblateClient,
	screenshot: Screenshot,
	wantedUnitIds: number[],
): Promise<void> {
	const attached = screenshot.unitIds;
	for (const unitId of wantedUnitIds.filter(id => !attached.includes(id))) {
		await weblate.attachScreenshotToUnit(screenshot.id, unitId);
	}
	for (const unitId of attached.filter(id => !wantedUnitIds.includes(id))) {
		await weblate.detachScreenshotFromUnit(screenshot.id, unitId);
	}
}

/** Upload the config's screenshots to its Weblate component, each attached to
 * the strings whose keys are saved next to it, and delete the ones this
 * uploaded before that are no longer there. The token to do it as comes from
 * WEBLATE_TOKEN: the one thing here that is a secret. */
export async function syncScreenshotsToWeblate(config: Config): Promise<void> {
	if (config.weblate === undefined) {
		throw new Error('The config names no Weblate component to sync to');
	}
	const token = process.env.WEBLATE_TOKEN;
	if (!token) throw new Error('WEBLATE_TOKEN is not set');
	const { url, project, component } = config.weblate;
	const { screenshots } = config;
	const weblate = new WeblateClient(url, token);

	refuseAPartialRun(screenshots);

	const units = await weblate.units(project, component, SOURCE_LANGUAGE);
	const locals: ScreenshotToSync[] = readLocalScreenshots(screenshots).map(
		local => ({
			...local,
			unitIds: units
				.filter(unit => local.keys.includes(renderedKey(unit.context)))
				.map(unit => unit.id),
		}),
	);
	// Weblate reports a screenshot attached to no string as a problem, so
	// one showing none of its strings is not uploaded, and is deleted if it
	// was before.
	const toSync = locals.filter(local => local.unitIds.length > 0);
	for (const local of locals.filter(local => local.unitIds.length === 0)) {
		console.warn(`${local.name}: shows no string Weblate has, skipped`);
	}
	const remotes = (await weblate.screenshots(project, component)).filter(
		screenshot => screenshot.name.startsWith(MANAGED_NAME_PREFIX),
	);

	for (const local of toSync) {
		const image = new Blob([readFileSync(local.image)], { type: 'image/png' });
		let screenshot = remotes.find(remote => remote.name === local.name);
		if (screenshot === undefined) {
			screenshot = await weblate.createScreenshot({
				project,
				component,
				language: SOURCE_LANGUAGE,
				name: local.name,
				image,
			});
		} else {
			await weblate.replaceScreenshotImage(screenshot.id, image);
		}
		await attachToUnits(weblate, screenshot, local.unitIds);
		console.log(`${local.name}: ${local.unitIds.length} strings`);
	}

	const localNames = new Set(toSync.map(local => local.name));
	for (const stale of remotes.filter(remote => !localNames.has(remote.name))) {
		await weblate.deleteScreenshot(stale.id);
		console.log(`${stale.name}: deleted`);
	}
}
