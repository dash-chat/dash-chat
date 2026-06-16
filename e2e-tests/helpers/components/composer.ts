import { tid } from '../selectors';

const TINY_PNG = [
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
	0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
	0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44,
	0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d,
	0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
	0x60, 0x82,
];

/** The shared message composer (text area + attachments) used by both
 * direct and group chats. */
export class Composer {
	constructor(private agent: WebdriverIO.Browser) {}

	messageInput = this.agent.$(tid('message-input-textarea'));
	sendButton = this.agent.$(tid('message-input-send'));
	mediaPreview = this.agent.$(tid('message-input-media-preview'));
	clearAttachments = this.agent.$(tid('message-input-clear-attachments'));
	addMoreTile = this.agent.$(tid('message-input-add-more'));

	removeAttachmentButton(index: number) {
		return this.agent.$(tid(`message-input-remove-attachment-${index}`));
	}

	/**
	 * Attach `count` photos (synthesized 1×1 PNGs) to the composer, named after
	 * `label` (`${label}-1.png`, …) so a specific send can later be matched with
	 * `waitForPhotoMessage(label)`. The hidden file input is populated via
	 * DataTransfer + a synthetic change event, the same trick add-contact uses
	 * for QR uploads.
	 */
	async attachPhotos(label: string, count = 1): Promise<void> {
		await this.agent.execute(
			(pngBytes: number[], n: number, name: string) => {
				const input = document.querySelector(
					'[data-testid="message-input-photo-picker"]',
				) as HTMLInputElement;
				const dt = new DataTransfer();
				for (let i = 1; i <= n; i++) {
					const blob = new Blob([new Uint8Array(pngBytes)], {
						type: 'image/png',
					});
					dt.items.add(
						new File([blob], `${name}-${i}.png`, { type: 'image/png' }),
					);
				}
				input.files = dt.files;
				input.dispatchEvent(new Event('change', { bubbles: true }));
			},
			TINY_PNG,
			count,
			label,
		);
		await this.mediaPreview.waitForExist({ timeout: 5_000 });
	}

	/** Attach a single non-image file to the composer. */
	async attachFile(
		name = 'notes.txt',
		contents = 'hello from e2e',
		mimeType = 'text/plain',
	): Promise<void> {
		await this.agent.execute(
			(n: string, c: string, m: string) => {
				const input = document.querySelector(
					'[data-testid="message-input-file-picker"]',
				) as HTMLInputElement;
				const dt = new DataTransfer();
				dt.items.add(new File([new Blob([c], { type: m })], n, { type: m }));
				input.files = dt.files;
				input.dispatchEvent(new Event('change', { bubbles: true }));
			},
			name,
			contents,
			mimeType,
		);
		await this.mediaPreview.waitForExist({ timeout: 5_000 });
	}

	/** Attach a zero-filled file of exactly `sizeBytes` to test the size cap. */
	async attachFileOfSize(sizeBytes: number, name = 'big.bin'): Promise<void> {
		await this.agent.execute(
			(size: number, n: string) => {
				const input = document.querySelector(
					'[data-testid="message-input-file-picker"]',
				) as HTMLInputElement;
				const dt = new DataTransfer();
				const blob = new Blob([new Uint8Array(size)], {
					type: 'application/octet-stream',
				});
				dt.items.add(new File([blob], n, { type: 'application/octet-stream' }));
				input.files = dt.files;
				input.dispatchEvent(new Event('change', { bubbles: true }));
			},
			sizeBytes,
			name,
		);
		await this.mediaPreview.waitForExist({ timeout: 5_000 });
	}

	/** Paste `count` synthesized PNGs into the composer textarea. */
	async pastePhotos(count = 1): Promise<void> {
		await this.agent.execute(
			(pngBytes: number[], n: number) => {
				const specs = Array.from({ length: n }, (_, i) => ({
					name: `pasted-${i + 1}.png`,
					mimeType: 'image/png',
					bytes: pngBytes,
				}));
				window.__test.pasteFiles(specs);
			},
			TINY_PNG,
			count,
		);
		await this.mediaPreview.waitForExist({ timeout: 5_000 });
	}

	/** Drop `count` synthesized PNGs onto the window (HTML5 drop pipeline). */
	async dropPhotos(count = 1): Promise<void> {
		await this.agent.execute(
			(pngBytes: number[], n: number) => {
				const specs = Array.from({ length: n }, (_, i) => ({
					name: `dropped-${i + 1}.png`,
					mimeType: 'image/png',
					bytes: pngBytes,
				}));
				window.__test.dropFiles(specs);
			},
			TINY_PNG,
			count,
		);
		await this.mediaPreview.waitForExist({ timeout: 5_000 });
	}

	/** Send the composer content. The send button only renders on mobile
	 * user agents, so on desktop (CI) dispatch Enter on the textarea the way
	 * a desktop user sends. Composer must already have content. */
	async send(): Promise<void> {
		if (await this.sendButton.isExisting()) {
			await this.sendButton.click();
			return;
		}
		await this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLTextAreaElement;
			el.focus();
			el.dispatchEvent(
				new KeyboardEvent('keydown', {
					key: 'Enter',
					code: 'Enter',
					bubbles: true,
					cancelable: true,
				}),
			);
		}, tid('message-input-textarea'));
	}

	/** Remove the currently-attached draft via the preview's remove button. */
	async removeDraft(): Promise<void> {
		await this.mediaPreview.$('button').click();
	}

	async hasMediaPreview(): Promise<boolean> {
		return this.mediaPreview.isExisting();
	}

	/** Number of photo thumbnails currently staged in the composer preview. */
	async stagedPhotoCount(): Promise<number> {
		return this.agent.execute(
			(sel: string) => document.querySelectorAll(`${sel} img`).length,
			tid('message-input-media-preview'),
		);
	}

	/** Wait until exactly `expected` photos are staged in the preview. */
	async expectStagedPhotoCount(expected: number): Promise<void> {
		await this.agent.waitUntil(
			async () => (await this.stagedPhotoCount()) === expected,
			{
				timeoutMsg: `Expected ${expected} staged photos, got ${await this.stagedPhotoCount()}`,
			},
		);
	}
}
