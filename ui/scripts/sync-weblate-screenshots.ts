import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

import { SCREENSHOTS_DIR, renderedKey } from './translation-keys.ts';

const SOURCE_LANGUAGE = 'en';
// Marks the screenshots this script owns, so the ones translators upload by
// hand are never replaced or deleted.
const MANAGED_NAME_PREFIX = 'e2e: ';

interface Page<T> {
	next: string | null;
	results: T[];
}

interface Unit {
	id: number;
	context: string;
}

interface Screenshot {
	id: number;
	name: string;
	/** URLs of the units the screenshot is attached to, each ending in the unit id. */
	units: string[];
}

interface LocalScreenshot {
	name: string;
	image: string;
	keys: string[];
}

function requiredEnv(name: string): string {
	const value = process.env[name];
	if (!value) throw new Error(`${name} is not set`);
	return value;
}

const WEBLATE_URL = requiredEnv('WEBLATE_URL').replace(/\/$/, '');
const WEBLATE_TOKEN = requiredEnv('WEBLATE_TOKEN');
const PROJECT = requiredEnv('WEBLATE_PROJECT');
const COMPONENT = requiredEnv('WEBLATE_COMPONENT');

async function request(
	url: string,
	init: { method: string; body?: FormData } = { method: 'GET' },
): Promise<Response> {
	const response = await fetch(
		url.startsWith('http') ? url : `${WEBLATE_URL}/api/${url}`,
		{
			...init,
			headers: {
				Authorization: `Token ${WEBLATE_TOKEN}`,
				Accept: 'application/json',
			},
		},
	);
	if (!response.ok) {
		throw new Error(
			`${init.method} ${url}: ${response.status} ${await response.text()}`,
		);
	}
	return response;
}

async function listAll<T>(url: string): Promise<T[]> {
	const results: T[] = [];
	let next: string | null = url;
	while (next !== null) {
		const page = (await (await request(next)).json()) as Page<T>;
		results.push(...page.results);
		next = page.next;
	}
	return results;
}

function form(fields: Record<string, string | Blob>): FormData {
	const data = new FormData();
	for (const [name, value] of Object.entries(fields)) {
		if (typeof value === 'string') data.append(name, value);
		else data.append(name, value, 'screenshot.png');
	}
	return data;
}

function readLocalScreenshots(): LocalScreenshot[] {
	return readdirSync(SCREENSHOTS_DIR)
		.filter(file => file.endsWith('.keys.json'))
		.map(file => {
			const name = file.slice(0, -'.keys.json'.length);
			return {
				name: `${MANAGED_NAME_PREFIX}${name}`,
				image: path.join(SCREENSHOTS_DIR, `${name}.png`),
				keys: JSON.parse(
					readFileSync(path.join(SCREENSHOTS_DIR, file), 'utf8'),
				) as string[],
			};
		});
}

function unitIdOfUrl(unitUrl: string): number {
	return Number(unitUrl.replace(/\/$/, '').split('/').pop());
}

/** Upload `local`'s image over `existing`, or as a new screenshot when there
 *  is none. Resolves to the screenshot as Weblate now has it. */
async function uploadImage(
	local: LocalScreenshot,
	existing: Screenshot | undefined,
): Promise<Screenshot> {
	const image = new Blob([readFileSync(local.image)], { type: 'image/png' });
	if (existing !== undefined) {
		await request(`screenshots/${existing.id}/file/`, {
			method: 'POST',
			body: form({ image }),
		});
		return existing;
	}
	const created = await request('screenshots/', {
		method: 'POST',
		body: form({
			image,
			name: local.name,
			project_slug: PROJECT,
			component_slug: COMPONENT,
			language_code: SOURCE_LANGUAGE,
		}),
	});
	return (await created.json()) as Screenshot;
}

/** Attach `screenshot` to exactly the units in `wantedUnitIds`. */
async function attachToUnits(
	screenshot: Screenshot,
	wantedUnitIds: number[],
): Promise<void> {
	const attached = screenshot.units.map(unitIdOfUrl);
	for (const unitId of wantedUnitIds.filter(id => !attached.includes(id))) {
		await request(`screenshots/${screenshot.id}/units/`, {
			method: 'POST',
			body: form({ unit_id: String(unitId) }),
		});
	}
	for (const unitId of attached.filter(id => !wantedUnitIds.includes(id))) {
		await request(`screenshots/${screenshot.id}/units/${unitId}`, {
			method: 'DELETE',
		});
	}
}

const locals = readLocalScreenshots();
if (locals.length === 0) {
	throw new Error(
		`No screenshots in ${SCREENSHOTS_DIR}: run \`just translation-screenshots\` first`,
	);
}

const units = await listAll<Unit>(
	`translations/${PROJECT}/${COMPONENT}/${SOURCE_LANGUAGE}/units/`,
);
const remotes = (
	await listAll<Screenshot>(`components/${PROJECT}/${COMPONENT}/screenshots/`)
).filter(screenshot => screenshot.name.startsWith(MANAGED_NAME_PREFIX));

for (const local of locals) {
	const screenshot = await uploadImage(
		local,
		remotes.find(remote => remote.name === local.name),
	);
	const unitIds = units
		.filter(unit => local.keys.includes(renderedKey(unit.context)))
		.map(unit => unit.id);
	await attachToUnits(screenshot, unitIds);
	console.log(`${local.name}: ${unitIds.length} strings`);
}

const localNames = new Set(locals.map(local => local.name));
for (const stale of remotes.filter(remote => !localNames.has(remote.name))) {
	await request(`screenshots/${stale.id}/`, { method: 'DELETE' });
	console.log(`${stale.name}: deleted`);
}
