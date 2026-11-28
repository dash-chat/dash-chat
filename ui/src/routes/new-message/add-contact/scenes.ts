import { scene, untilAToastShows } from '$lib/scenes';
import { TINY_PNG_BYTES } from '$lib/scenes/demo-world';
import { type Page, expect } from '@playwright/test';
import { backend } from 'dash-chat-mock';

/** Put the pictures the app saves on the page, where a browser would hand
 * them to its downloads. */
async function showDownloadsOnThePage(page: Page): Promise<void> {
	await page.evaluate(() => {
		HTMLAnchorElement.prototype.click = function (this: HTMLAnchorElement) {
			const saved = document.createElement('img');
			saved.src = this.href;
			saved.style.cssText =
				'position: fixed; inset: 0; margin: auto; max-height: 100%; max-width: 100%; z-index: 100000;';
			document.body.appendChild(saved);
		};
	});
}

async function openMyOwnLink(page: Page): Promise<void> {
	const myLink = () =>
		page
			.locator('wa-qr-code')
			.first()
			.evaluate((code: HTMLElement & { value: string }) => code.value);
	await expect.poll(myLink).toBeTruthy();
	await page.evaluate(
		link => window.__test.handleDeepLink(link),
		await myLink(),
	);
}

function addingFailsWith(kind: string): void {
	backend.fails('add_contact', kind);
}

scene('error-adding-yourself', async ({ page, open }) => {
	addingFailsWith('CannotAddSelf');
	await open('/new-message/add-contact');
	await openMyOwnLink(page);
});

scene('error-invalid-contact-link', async ({ page, open }) => {
	addingFailsWith('InvalidContactCode');
	await open('/new-message/add-contact');
	await openMyOwnLink(page);
});

scene('error-adding-contact-without-profile', async ({ page, open }) => {
	addingFailsWith('ProfileNotCreated');
	await open('/new-message/add-contact');
	await openMyOwnLink(page);
});

scene('error-adding-contact', async ({ page, open }) => {
	addingFailsWith('StoreContact');
	await open('/new-message/add-contact');
	await openMyOwnLink(page);
});

scene('new-message-add-contact', async ({ open }) => {
	await open('/new-message/add-contact');
});

scene('add-contact-link-sheet', async ({ page, open }) => {
	await open('/new-message/add-contact');
	await page.getByTestId('add-contact-link-btn').first().click();
});

scene('add-contact-link-copied', async ({ page, open }) => {
	await open('/new-message/add-contact');
	await page.getByTestId('add-contact-link-btn').first().click();
	await page.getByTestId('qr-link-sheet-copy').first().click();
});

scene('add-contact-scan', async ({ page, open }) => {
	await open('/new-message/add-contact');
	await page.getByTestId('add-contact-scan-tab').first().click();
});

scene('error-creating-contact-code', async ({ open }) => {
	backend.fails('create_contact_code');
	await open('/new-message/add-contact');
});

scene('error-unrecognized-link', async ({ page, open }) => {
	await open('/new-message/add-contact');
	await page.evaluate(() =>
		window.__test.handleDeepLink('https://dashchat.org/nowhere'),
	);
});

scene(
	'desktop-add-contact',
	async ({ open }) => {
		await open('/new-message/add-contact');
	},
	{ device: 'desktop' },
);

scene('error-no-qr-code-in-image', async ({ page, open }) => {
	await open('/new-message/add-contact');
	await page.getByTestId('add-contact-file-input').setInputFiles({
		name: 'blank.png',
		mimeType: 'image/png',
		buffer: Buffer.from(TINY_PNG_BYTES),
	});
	await untilAToastShows(page);
});

scene(
	'desktop-saved-qr-code-picture',
	async ({ page, open }) => {
		await open('/new-message/add-contact');
		await showDownloadsOnThePage(page);
		await page.getByTestId('add-contact-save-btn').first().click();
	},
	{ device: 'desktop', drawn: ['shareQrCodeSubtitle'] },
);
