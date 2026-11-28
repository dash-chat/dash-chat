import type {
	FullResult,
	Reporter,
	TestCase,
	TestResult,
} from '@playwright/test/reporter';
import {
	type NamedScreenshot,
	syncScreenshotsToWeblate,
} from 'i18n-screenshots-weblate';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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

/** Save the screenshots, and the keys each shows, for someone to look at. */
function save(screenshots: NamedScreenshot[]): void {
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

/** Whether every source string is shown by a scene or listed as one no scene
 *  can show, and none is listed for nothing: the list is how a new string
 *  without a screenshot gets noticed. */
function everyStringIsAccountedFor(shownKeys: Set<string>): boolean {
	const sourceKeys = new Set(
		renderedSourceKeys(
			JSON.parse(readFileSync(SOURCE_STRINGS, 'utf8')) as Record<
				string,
				string
			>,
		),
	);
	const listed = new Set(
		JSON.parse(readFileSync(STRINGS_WITHOUT_SCREENSHOT, 'utf8')) as string[],
	);
	const notShown = sourceKeys.difference(shownKeys);
	console.log(
		`${sourceKeys.size - notShown.size}/${sourceKeys.size} source strings have a screenshot`,
	);

	const unlisted = notShown.difference(listed);
	const stale = listed.difference(notShown);
	if (unlisted.size > 0) {
		console.error(
			`No scene shows these strings; add a scene, or list them in ${STRINGS_WITHOUT_SCREENSHOT}: ${[...unlisted].join(', ')}`,
		);
	}
	if (stale.size > 0) {
		console.error(
			`Listed in ${STRINGS_WITHOUT_SCREENSHOT} but shown by a scene or gone from en.json; remove them: ${[...stale].join(', ')}`,
		);
	}
	return unlisted.size === 0 && stale.size === 0;
}

/** Collects the screenshot each scene attaches, and once they are all taken
 * saves them, checks that they cover the source strings, and syncs them to
 * Weblate when `I18N_SCREENSHOTS_SYNC` asks for it. */
export default class ScreenshotsReporter implements Reporter {
	private screenshots: NamedScreenshot[] = [];

	onTestEnd(test: TestCase, result: TestResult): void {
		const attached = (name: string) =>
			result.attachments.find(attachment => attachment.name === name)?.body;
		const image = attached('screenshot');
		const keys = attached('keys');
		if (result.status !== 'passed' || !image || !keys) return;
		this.screenshots.push({
			name: test.title,
			image,
			keys: JSON.parse(keys.toString()) as string[],
		});
	}

	async onEnd(result: FullResult): Promise<{ status: FullResult['status'] }> {
		save(this.screenshots);
		// A run of some scenes neither covers every string nor may be synced:
		// Weblate deletes the screenshots it is not handed.
		if (process.env.SCENES || result.status !== 'passed') return result;

		const shownKeys = new Set(this.screenshots.flatMap(({ keys }) => keys));
		if (!everyStringIsAccountedFor(shownKeys)) return { status: 'failed' };

		if (process.env.I18N_SCREENSHOTS_SYNC) {
			const token = process.env.WEBLATE_TOKEN;
			if (!token) throw new Error('WEBLATE_TOKEN is not set');
			await syncScreenshotsToWeblate({
				screenshots: this.screenshots,
				url: 'https://nuance.weblate.cloud',
				token,
				project: 'dash-chat',
				component: 'dash-chat-app',
				sourceLanguage: 'en',
				renderedKey,
				log: console.log,
			});
		}
		return result;
	}
}
