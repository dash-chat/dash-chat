/**
 * Media-attachment helpers for E2E tests. The composer's two file inputs are
 * hidden — instead of opening a native picker we set `input.files` via
 * `DataTransfer` and dispatch a synthetic `change` event, the same trick used
 * by `add-contact` for QR uploads.
 */

import { S } from '../selectors';
import { click, nextTick, waitFor } from '../helpers';

function setHiddenFileInput(selector: string, files: File[]): void {
	const input = document.querySelector(selector) as HTMLInputElement | null;
	if (!input) throw new Error(`file input not found: ${selector}`);
	const dt = new DataTransfer();
	for (const f of files) dt.items.add(f);
	input.files = dt.files;
	input.dispatchEvent(new Event('change', { bubbles: true }));
}

/** A 1×1 transparent PNG. Smallest valid image we can synthesize without canvas. */
const TINY_PNG_BYTES = new Uint8Array([
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
	0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
	0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44,
	0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d,
	0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
	0x60, 0x82,
]);

/**
 * Attach one or more photos (synthesized 1×1 PNGs) to the open composer.
 * Names default to `photo-1.png`, `photo-2.png`, ...
 */
export async function attachPhotos(count = 1): Promise<true> {
	const files: File[] = [];
	for (let i = 1; i <= count; i++) {
		const blob = new Blob([TINY_PNG_BYTES], { type: 'image/png' });
		files.push(new File([blob], `photo-${i}.png`, { type: 'image/png' }));
	}
	setHiddenFileInput(S.messageInput.photoPicker, files);
	await waitFor(S.messageInput.mediaPreview, 5_000);
	return true;
}

/** Attach a single non-image file. Defaults to a tiny `notes.txt`. */
export async function attachFile(
	name = 'notes.txt',
	contents = 'hello from e2e',
	mimeType = 'text/plain',
): Promise<true> {
	const blob = new Blob([contents], { type: mimeType });
	const file = new File([blob], name, { type: mimeType });
	setHiddenFileInput(S.messageInput.filePicker, [file]);
	await waitFor(S.messageInput.mediaPreview, 5_000);
	return true;
}

/** Click send. Composer must already have content (text and/or media). */
export async function sendComposer(): Promise<void> {
	await nextTick();
	click(S.messageInput.send);
}

/** Wait until a photo-bearing message appears anywhere in the chat. */
export function waitForPhotoMessage(timeout = 25_000): Promise<true> {
	return new Promise((resolve, reject) => {
		const t = setTimeout(
			() => reject(new Error('Timeout waiting for photo message')),
			timeout,
		);
		const check = () => {
			const root = document.querySelector(S.directChat.messages);
			const img = root?.querySelector(
				`${S.messageAttachment.photos} img`,
			) as HTMLImageElement | null;
			if (img && (img.complete ? img.naturalWidth > 0 : false)) {
				clearTimeout(t);
				resolve(true);
			} else {
				setTimeout(check, 100);
			}
		};
		check();
	});
}

/** Wait until a file-bearing message with the given filename appears. */
export function waitForFileMessage(name: string, timeout = 25_000): Promise<true> {
	return new Promise((resolve, reject) => {
		const t = setTimeout(
			() => reject(new Error(`Timeout waiting for file "${name}"`)),
			timeout,
		);
		const check = () => {
			const root = document.querySelector(S.directChat.messages);
			const buttons = root?.querySelectorAll(S.messageAttachment.file) ?? [];
			for (const btn of Array.from(buttons)) {
				if (btn.textContent?.includes(name)) {
					clearTimeout(t);
					resolve(true);
					return;
				}
			}
			setTimeout(check, 100);
		};
		check();
	});
}
