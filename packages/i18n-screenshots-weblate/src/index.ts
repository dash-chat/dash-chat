import type { Screenshot } from 'i18n-screenshots/screenshot';

import {
	type Screenshot as RemoteScreenshot,
	WeblateClient,
} from './weblate-client.ts';

// Marks the screenshots this owns, so the ones translators upload by hand
// are never replaced or deleted.
const MANAGED_NAME_PREFIX = 'scene: ';

export interface WeblateSyncOptions {
	/** One screenshot of every scene the app has. */
	screenshots: Screenshot[];
	/** The names of every scene the app has, to tell a partial set by. */
	sceneNames: string[];
	/** The address of the Weblate instance. */
	url: string;
	/** A token allowed to manage the component's screenshots. */
	token: string;
	project: string;
	component: string;
	sourceLanguage: string;
	/** The key the app renders a Weblate string under, where its translation
	 * file names it otherwise: a plural form with a suffix of its own, say. */
	renderedKey?: (weblateKey: string) => string;
	/** Told what is done to each screenshot, to follow a long run. */
	log?: (line: string) => void;
}

/** A screenshot and the Weblate strings it shows. */
interface ScreenshotToSync extends Screenshot {
	managedName: string;
	unitIds: number[];
}

/** Throws unless there is a screenshot of every scene: what is missing would
 * be deleted from Weblate as no longer existing. */
function refuseAPartialSet(
	screenshots: Screenshot[],
	sceneNames: string[],
): void {
	const missing = sceneNames.filter(
		name => !screenshots.some(screenshot => screenshot.name === name),
	);
	if (missing.length > 0) {
		throw new Error(
			`Not syncing: there is no screenshot of ${missing.length} scenes, which would be deleted from Weblate. Take them all first. Missing: ${missing.join(', ')}`,
		);
	}
}

/** Attach `screenshot` to exactly the units in `wantedUnitIds`. */
async function attachToUnits(
	weblate: WeblateClient,
	screenshot: RemoteScreenshot,
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

/** Upload the screenshots to the Weblate component, each attached to the
 * strings it shows, and delete the ones this uploaded before that are no
 * longer among them. */
export async function syncScreenshotsToWeblate({
	screenshots,
	sceneNames,
	url,
	token,
	project,
	component,
	sourceLanguage,
	renderedKey = weblateKey => weblateKey,
	log = () => {},
}: WeblateSyncOptions): Promise<void> {
	refuseAPartialSet(screenshots, sceneNames);

	const weblate = new WeblateClient(url, token);
	const units = await weblate.units(project, component, sourceLanguage);
	if (units.length === 0) {
		throw new Error(
			`Not syncing: Weblate lists no strings for ${project}/${component}, so no screenshot could be attached to any`,
		);
	}
	const locals: ScreenshotToSync[] = screenshots.map(screenshot => ({
		...screenshot,
		managedName: `${MANAGED_NAME_PREFIX}${screenshot.name}`,
		unitIds: units
			.filter(unit => screenshot.keys.includes(renderedKey(unit.context)))
			.map(unit => unit.id),
	}));
	const remotes = (await weblate.screenshots(project, component)).filter(
		remote => remote.name.startsWith(MANAGED_NAME_PREFIX),
	);

	for (const local of locals) {
		// Weblate reports a screenshot attached to no string as a problem, so
		// one showing none of its strings is not uploaded. What was uploaded
		// of it before stays: Weblate may only be behind on the strings.
		if (local.unitIds.length === 0) {
			log(`${local.managedName}: shows no string Weblate has, skipped`);
			continue;
		}
		const image = new Blob([new Uint8Array(local.image)], {
			type: 'image/png',
		});
		let remote = remotes.find(remote => remote.name === local.managedName);
		if (remote === undefined) {
			remote = await weblate.createScreenshot({
				project,
				component,
				language: sourceLanguage,
				name: local.managedName,
				image,
			});
		} else {
			await weblate.replaceScreenshotImage(remote.id, image);
		}
		await attachToUnits(weblate, remote, local.unitIds);
		log(`${local.managedName}: ${local.unitIds.length} strings`);
	}

	const managedNames = new Set(locals.map(local => local.managedName));
	for (const stale of remotes.filter(
		remote => !managedNames.has(remote.name),
	)) {
		await weblate.deleteScreenshot(stale.id);
		log(`${stale.name}: deleted`);
	}
}
