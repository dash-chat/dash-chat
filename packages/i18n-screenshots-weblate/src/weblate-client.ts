interface Page<T> {
	next: string | null;
	results: T[];
}

/** A string of a component in one language. */
export interface Unit {
	id: number;
	/** The key of the string in the translation file. */
	context: string;
}

export interface Screenshot {
	id: number;
	name: string;
	unitIds: number[];
}

/** A screenshot as the API spells it. Creating one answers without an `id`,
 * so the id is always read off `url`. */
interface ScreenshotResponse {
	url: string;
	name: string;
	/** URLs of the units it is attached to. */
	units: string[];
}

export interface NewScreenshot {
	project: string;
	component: string;
	language: string;
	name: string;
	image: Blob;
}

function idAtTheEndOf(url: string): number {
	return Number(url.replace(/\/$/, '').split('/').pop());
}

function screenshotOf(response: ScreenshotResponse): Screenshot {
	return {
		id: idAtTheEndOf(response.url),
		name: response.name,
		unitIds: response.units.map(idAtTheEndOf),
	};
}

function form(fields: Record<string, string | Blob>): FormData {
	const data = new FormData();
	for (const [name, value] of Object.entries(fields)) {
		if (typeof value === 'string') data.append(name, value);
		else data.append(name, value, 'screenshot.png');
	}
	return data;
}

export class WeblateClient {
	private readonly api: string;
	private readonly token: string;

	/** `url` is the address of the Weblate instance, `token` one allowed to
	 * manage the screenshots of the projects asked about. */
	constructor(url: string, token: string) {
		this.api = `${url.replace(/\/$/, '')}/api/`;
		this.token = token;
	}

	units(project: string, component: string, language: string): Promise<Unit[]> {
		return this.listAll<Unit>(
			`translations/${project}/${component}/${language}/units/`,
		);
	}

	async screenshots(project: string, component: string): Promise<Screenshot[]> {
		const screenshots = await this.listAll<ScreenshotResponse>(
			`components/${project}/${component}/screenshots/`,
		);
		return screenshots.map(screenshotOf);
	}

	async createScreenshot(screenshot: NewScreenshot): Promise<Screenshot> {
		const created = await this.request('screenshots/', {
			method: 'POST',
			body: form({
				image: screenshot.image,
				name: screenshot.name,
				project_slug: screenshot.project,
				component_slug: screenshot.component,
				language_code: screenshot.language,
			}),
		});
		return screenshotOf((await created.json()) as ScreenshotResponse);
	}

	async replaceScreenshotImage(
		screenshotId: number,
		image: Blob,
	): Promise<void> {
		await this.request(`screenshots/${screenshotId}/file/`, {
			method: 'POST',
			body: form({ image }),
		});
	}

	async attachScreenshotToUnit(
		screenshotId: number,
		unitId: number,
	): Promise<void> {
		await this.request(`screenshots/${screenshotId}/units/`, {
			method: 'POST',
			body: form({ unit_id: String(unitId) }),
		});
	}

	async detachScreenshotFromUnit(
		screenshotId: number,
		unitId: number,
	): Promise<void> {
		await this.request(`screenshots/${screenshotId}/units/${unitId}`, {
			method: 'DELETE',
		});
	}

	async deleteScreenshot(screenshotId: number): Promise<void> {
		await this.request(`screenshots/${screenshotId}/`, { method: 'DELETE' });
	}

	private async request(
		url: string,
		init: { method: string; body?: FormData } = { method: 'GET' },
	): Promise<Response> {
		const response = await fetch(
			url.startsWith('http') ? url : `${this.api}${url}`,
			{
				...init,
				headers: {
					Authorization: `Token ${this.token}`,
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

	/** Every result of a listing, following its pages. */
	private async listAll<T>(url: string): Promise<T[]> {
		const results: T[] = [];
		let next: string | null = url;
		while (next !== null) {
			const page = (await (await this.request(next)).json()) as Page<T>;
			results.push(...page.results);
			next = page.next;
		}
		return results;
	}
}
