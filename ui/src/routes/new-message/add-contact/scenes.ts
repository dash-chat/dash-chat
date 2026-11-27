import type { Scene } from '$lib/scenes/scene';
import { open, untilAToastShows } from '$lib/scenes/steps';
import { setMockBackend } from 'dash-chat-mock';
import { present, settled, tap } from 'i18n-screenshots/scene';

/** Hand the upload input a picture with no QR code in it, the way picking
 * one would. */
async function uploadABlankImage(): Promise<void> {
	const canvas = document.createElement('canvas');
	canvas.width = 64;
	canvas.height = 64;
	canvas.getContext('2d')?.fillRect(0, 0, 64, 64);
	const blank = await new Promise<Blob | null>(resolve =>
		canvas.toBlob(resolve),
	);
	const input = await present<HTMLInputElement>(
		'[data-testid="add-contact-file-input"]',
	);
	if (blank === null) return;
	const picked = new DataTransfer();
	picked.items.add(new File([blank], 'blank.png', { type: 'image/png' }));
	input.files = picked.files;
	input.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Put the pictures the app saves on the page, where a browser would hand
 * them to its downloads. */
function showDownloadsOnThePage(): void {
	HTMLAnchorElement.prototype.click = function (this: HTMLAnchorElement) {
		const saved = document.createElement('img');
		saved.src = this.href;
		saved.style.cssText =
			'position: fixed; inset: 0; margin: auto; max-height: 100%; max-width: 100%; z-index: 100000;';
		document.body.appendChild(saved);
	};
}

async function openMyOwnLink(): Promise<void> {
	const myCode = await present<HTMLElement & { value: string }>('wa-qr-code');
	window.__test.handleDeepLink(myCode.value);
}

function addingFailsWith(kind: string): void {
	setMockBackend({ failingCommands: [`add_contact:${kind}`] });
}

export default [
	{
		name: 'error-adding-yourself',
		async setup() {
			addingFailsWith('CannotAddSelf');
			await open('/new-message/add-contact');
			await openMyOwnLink();
		},
	},
	{
		name: 'error-invalid-contact-link',
		async setup() {
			addingFailsWith('InvalidContactCode');
			await open('/new-message/add-contact');
			await openMyOwnLink();
		},
	},
	{
		name: 'error-adding-contact-without-profile',
		async setup() {
			addingFailsWith('ProfileNotCreated');
			await open('/new-message/add-contact');
			await openMyOwnLink();
		},
	},
	{
		name: 'error-adding-contact',
		async setup() {
			addingFailsWith('StoreContact');
			await open('/new-message/add-contact');
			await openMyOwnLink();
		},
	},
	{
		name: 'new-message-add-contact',
		setup: () => open('/new-message/add-contact'),
	},
	{
		name: 'add-contact-link-sheet',
		async setup() {
			await open('/new-message/add-contact');
			await tap('add-contact-link-btn');
		},
	},
	{
		name: 'add-contact-link-copied',
		async setup() {
			await open('/new-message/add-contact');
			await tap('add-contact-link-btn');
			await tap('qr-link-sheet-copy');
		},
	},
	{
		name: 'add-contact-scan',
		async setup() {
			await open('/new-message/add-contact');
			await tap('add-contact-scan-tab');
		},
	},
	{
		name: 'error-creating-contact-code',
		async setup() {
			setMockBackend({ failingCommands: ['create_contact_code'] });
			await open('/new-message/add-contact');
		},
	},
	{
		name: 'error-unrecognized-link',
		async setup() {
			await open('/new-message/add-contact');
			window.__test.handleDeepLink('https://dashchat.org/nowhere');
		},
	},
	{
		name: 'desktop-add-contact',
		device: 'desktop',
		setup: () => open('/new-message/add-contact'),
	},
	{
		name: 'error-no-qr-code-in-image',
		async setup() {
			await open('/new-message/add-contact');
			await uploadABlankImage();
			await settled();
			await untilAToastShows();
		},
	},
	{
		name: 'desktop-saved-qr-code-picture',
		device: 'desktop',
		drawn: ['shareQrCodeSubtitle'],
		async setup() {
			await open('/new-message/add-contact');
			showDownloadsOnThePage();
			await settled();
			await tap('add-contact-save-btn');
		},
	},
] satisfies Scene[];
