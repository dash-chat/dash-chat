import type { Scene } from '$lib/scenes/scene';
import { untilAToastShows } from '$lib/scenes/steps';
import { mockBackend } from 'dash-chat-stores';

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
	const input = document.querySelector<HTMLInputElement>(
		'[data-testid="add-contact-file-input"]',
	);
	if (blank === null || input === null) return;
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

function openMyOwnLink(): void {
	const myCode = document.querySelector<HTMLElement & { value: string }>(
		'wa-qr-code',
	);
	if (myCode !== null) window.__test.handleDeepLink(myCode.value);
}

function addingFailsWith(kind: string): () => void {
	return () => {
		mockBackend.failingCommands = [`add_contact:${kind}`];
	};
}

export default [
	{
		name: 'error-adding-yourself',
		path: '/new-message/add-contact',
		seed: addingFailsWith('CannotAddSelf'),
		steps: [openMyOwnLink],
	},
	{
		name: 'error-invalid-contact-link',
		path: '/new-message/add-contact',
		seed: addingFailsWith('InvalidContactCode'),
		steps: [openMyOwnLink],
	},
	{
		name: 'error-adding-contact-without-profile',
		path: '/new-message/add-contact',
		seed: addingFailsWith('ProfileNotCreated'),
		steps: [openMyOwnLink],
	},
	{
		name: 'error-adding-contact',
		path: '/new-message/add-contact',
		seed: addingFailsWith('StoreContact'),
		steps: [openMyOwnLink],
	},
	{ name: 'new-message-add-contact', path: '/new-message/add-contact' },
	{
		name: 'add-contact-link-sheet',
		path: '/new-message/add-contact',
		steps: [{ tap: 'add-contact-link-btn' }],
	},
	{
		name: 'add-contact-link-copied',
		path: '/new-message/add-contact',
		steps: [{ tap: 'add-contact-link-btn' }, { tap: 'qr-link-sheet-copy' }],
	},
	{
		name: 'add-contact-scan',
		path: '/new-message/add-contact',
		steps: [{ tap: 'add-contact-scan-tab' }],
	},
	{
		name: 'error-creating-contact-code',
		path: '/new-message/add-contact',
		seed() {
			mockBackend.failingCommands = ['create_contact_code'];
		},
	},
	{
		name: 'error-unrecognized-link',
		path: '/new-message/add-contact',
		steps: [() => window.__test.handleDeepLink('https://dashchat.org/nowhere')],
	},
	{
		name: 'desktop-add-contact',
		path: '/new-message/add-contact',
		device: 'desktop',
	},
	{
		name: 'error-no-qr-code-in-image',
		path: '/new-message/add-contact',
		steps: [uploadABlankImage, untilAToastShows],
	},
	{
		name: 'desktop-saved-qr-code-picture',
		path: '/new-message/add-contact',
		device: 'desktop',
		steps: [showDownloadsOnThePage, { tap: 'add-contact-save-btn' }],
		drawn: ['shareQrCodeSubtitle'],
	},
] satisfies Scene[];
