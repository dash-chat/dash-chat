import { type Page, devices, expect, test } from '@playwright/test';
import { seedDemoData } from 'dash-chat-mock';
import { takeScreenshot } from 'i18n-screenshots/capture';

import { tid } from '../../../tests/selectors';
import './node-storage';
import { clearStorage, storedEntries } from './node-storage';

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

interface Stage {
	page: Page;
	/** Start the app at `path`, on what the scene has written to the mock
	 * backend so far. */
	open: (path: string) => Promise<void>;
}

interface SceneOptions {
	/** What to render it as; an Android phone unless said otherwise. */
	device?: keyof typeof DEVICES;
	/** Keys of the strings it shows as pixels, which no marker can report:
	 * text the app draws into a picture. */
	drawn?: string[];
}

/** One state of the UI worth showing a translator, named `name`: `play`
 * brings the browser preview there, writing to the mock backend what it needs
 * on top of the demo data, opening the app, and doing to the page what a user
 * would. The screenshot is taken when it is done. */
export function scene(
	name: string,
	play: (stage: Stage) => void | Promise<void> = () => {},
	{ device = 'phone', drawn = [] }: SceneOptions = {},
): void {
	test(name, async ({ browser, baseURL }, testInfo) => {
		clearStorage();
		seedDemoData();
		const context = await browser.newContext({ ...DEVICES[device], baseURL });
		const page = await context.newPage();
		let opened = false;
		const open = async (path: string) => {
			await page.addInitScript(({ local, session }) => {
				for (const [key, value] of local) localStorage.setItem(key, value);
				for (const [key, value] of session) sessionStorage.setItem(key, value);
			}, storedEntries());
			await page.goto(path);
			// What scenes reach into the app through is registered after it starts.
			await page.waitForFunction(() => window.__test !== undefined);
			// An entry of its own to go back to: a scene that goes back in history
			// would otherwise leave the app for the blank page the tab opened on.
			await page.evaluate(() =>
				history.pushState(history.state, '', location.href),
			);
			opened = true;
		};
		await play({ page, open });
		if (!opened) await open('/');

		const { image, keys } = await takeScreenshot(page);
		// The app's catch-all error means the scene broke, unless the scene is of it.
		if (!name.startsWith('error-'))
			expect(keys).not.toContain('errorUnexpected');
		await testInfo.attach('screenshot', {
			body: Buffer.from(image),
			contentType: 'image/png',
		});
		await testInfo.attach('keys', {
			body: JSON.stringify([...keys, ...drawn]),
			contentType: 'application/json',
		});
		await context.close();
	});
}

/** The field with `testId`, or the one inside what has it. */
export function field(page: Page, testId: string) {
	return page
		.locator(
			`${tid(testId)}:is(input, textarea), ${tid(testId)} :is(input, textarea)`,
		)
		.first();
}

/** Keep a finger on the first element with `testId`, as a long-press does. */
export async function hold(page: Page, testId: string): Promise<void> {
	await page.getByTestId(testId).first().waitFor();
	await page.evaluate(
		selector => window.__test.longPress(selector),
		tid(testId),
	);
}

/** Waits for a toast to say something, for what the app only reports after
 * work of its own, like decoding an image. */
export async function untilAToastShows(page: Page): Promise<void> {
	await expect(page.getByTestId('toast').first()).toHaveText(/\S/);
}
