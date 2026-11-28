import type { Screenshot } from 'i18n-screenshots/capture';

import {
	type Screenshot as RemoteScreenshot,
	WeblateClient,
} from './weblate-client.ts';

// Marks the screenshots this owns, so the ones translators upload by hand
// are never replaced or deleted.
const MANAGED_NAME_PREFIX = 'scene: ';

export interface NamedScreenshot extends Screenshot {
	/** What Weblate knows it by from one sync to the next. */
	name: string;
}

export interface WeblateSyncOptions {
	/** Every screenshot there is: a managed screenshot in Weblate that is not
	 * among them is deleted as no longer existing. */
	screenshots: NamedScreenshot[];
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
interface ScreenshotToSync extends NamedScreenshot {
	managedName: string;
	unitIds: number[];
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
	url,
	token,
	project,
	component,
	sourceLanguage,
	renderedKey = weblateKey => weblateKey,
	log = () => {},
}: WeblateSyncOptions): Promise<void> {
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
