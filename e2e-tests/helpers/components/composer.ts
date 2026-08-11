import { TINY_PNG_BYTES } from '../images';
import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { SYNC_TIMEOUT } from '../timeouts';
import { RecentPhotosStrip } from './recent-photos-strip';

/** The shared message composer (text area + attachments) used by both
 * direct and group chats. */
export class Composer extends TestHelper {
	messageInput = this.el(tid('message-input-textarea'));
	sendButton = this.el(tid('message-input-send'));
	mediaPreview = this.el(tid('message-input-media-preview'));
	clearAttachments = this.el(tid('message-input-clear-attachments'));
	addMoreTile = this.el(tid('message-input-add-more'));
	editingBanner = this.el(tid('composer-editing-banner'));
	cancelEditButton = this.el(tid('composer-cancel-edit'));
	discardDraftDialog = this.el(tid('composer-discard-draft-dialog'));
	discardDraftCancel = this.el(tid('composer-discard-draft-cancel'));
	discardDraftConfirm = this.el(tid('composer-discard-draft-confirm'));
	attachButton = this.el(tid('message-input-attach'));
	mediaPanel = this.el(tid('message-input-media-panel'));
	recentPhotos = new RecentPhotosStrip(this.agent);

	attachMenuTrigger = this.el(tid('message-input-attach'));
	attachMenu = this.el(tid('message-input-attach-menu'));
	attachPhotosItem = this.el(tid('message-input-attach-photos'));
	attachFileItem = this.el(tid('message-input-attach-file'));
	stagedMediaPage = this.el(tid('staged-media-page'));

	removeAttachmentButton(index: number) {
		return this.agent.$(tid(`message-input-remove-attachment-${index}`));
	}

	/** The text currently in the composer. Read off the DOM property rather
	 * than with `getValue()`: on a mobile session that reads the `value`
	 * attribute, which a `<textarea>` does not have. */
	inputText(): Promise<string> {
		return this.agent.execute(
			(sel: string) =>
				document.querySelector<HTMLTextAreaElement>(sel)?.value ?? '',
			tid('message-input-textarea'),
		);
	}

	/** Wait for the staged-media UI: the inline preview on desktop, the
	 * full-screen staged-media page on mobile. */
	async waitForStagedMedia(): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				(await this.mediaPreview.isExisting()) ||
				(await this.stagedMediaPage.isExisting()),
			{ timeout: 5_000, timeoutMsg: 'Staged media UI did not appear' },
		);
	}

	/** Close the mobile staged-media page, discarding the staged draft. Android
	 * has no close button — popping the history entry is the hardware-back path
	 * that works on every mobile platform. */
	private async closeStagedMediaPage(): Promise<void> {
		await this.agent.execute(() => history.back());
		await this.stagedMediaPage.waitForExist({ reverse: true });
	}

	/** Open the mobile media panel via the attach button. Returns false when the
	 * panel isn't available (desktop user agents show the MediaMenu instead). */
	async openMediaPanel(): Promise<boolean> {
		await this.attachButton.click();
		try {
			await this.mediaPanel.waitForExist({ timeout: 2_000 });
			return true;
		} catch {
			return false;
		}
	}

	/**
	 * Open the desktop attach dropdown by clicking its trigger. The dropdown
	 * renders only on non-mobile builds (which CI is), where it replaces the
	 * mobile media panel. Resolves once the Photos item is visible.
	 */
	async openAttachMenu(): Promise<void> {
		await this.attachMenuTrigger.click();
		await this.attachPhotosItem.waitForDisplayed();
	}

	/** Close the attach dropdown by toggling its trigger. */
	async closeAttachMenu(): Promise<void> {
		await this.attachMenuTrigger.click();
		await this.attachPhotosItem.waitForDisplayed({ reverse: true });
	}

	/**
	 * Trimmed label of an attach-menu item, read via `textContent`. The items
	 * are `wa-dropdown-item` web components whose label is a slotted text node,
	 * and WebKitGTK's WebDriver `getText` returns empty for such hosts.
	 */
	async attachItemLabel(item: 'photos' | 'file'): Promise<string> {
		const testid =
			item === 'photos'
				? 'message-input-attach-photos'
				: 'message-input-attach-file';
		return this.agent.execute(
			(sel: string) => document.querySelector(sel)?.textContent?.trim() ?? '',
			tid(testid),
		);
	}

	/**
	 * Stage a single synthesized 1×1 PNG named `${label}.png` so a later send can
	 * be matched with `waitForPhotoMessage(label)`. Injected through the paste
	 * pipeline — the native file picker can't be driven headlessly.
	 */
	async attachPhotos(label: string): Promise<void> {
		await this.pastePhotos(label);
	}

	/** Attach a single non-image file to the composer. */
	async attachFile(
		name = 'notes.txt',
		contents = 'hello from e2e',
		mimeType = 'text/plain',
	): Promise<void> {
		await this.messageInput.waitForExist();
		await this.agent.execute(
			(n: string, c: string, m: string) => {
				const bytes = Array.from(new TextEncoder().encode(c));
				window.__test.pasteFiles([{ name: n, mimeType: m, bytes }]);
			},
			name,
			contents,
			mimeType,
		);
		await this.waitForStagedMedia();
	}

	/** Attach a zero-filled file of exactly `sizeBytes`. */
	async attachFileOfSize(sizeBytes: number, name = 'big.bin'): Promise<void> {
		await this.messageInput.waitForExist();
		await this.agent.execute(
			(size: number, n: string) => {
				window.__test.pasteFiles([
					{ name: n, mimeType: 'application/octet-stream', size },
				]);
			},
			sizeBytes,
			name,
		);
		await this.waitForStagedMedia();
	}

	/** Stage a synthesized noise JPEG named `${label}.jpg` at the given pixel
	 * size. Encoding is async in the page, so the staged-media wait is what
	 * confirms the paste actually landed. */
	async attachNoisePhoto(
		label: string,
		width: number,
		height: number,
	): Promise<void> {
		await this.messageInput.waitForExist();
		await this.agent.execute(
			async (name: string, w: number, h: number) => {
				await window.__test.pasteNoisePhoto({ name, width: w, height: h });
			},
			`${label}.jpg`,
			width,
			height,
		);
		await this.waitForStagedMedia();
	}

	/**
	 * Stage a synthetic voice note in the composer. Microphone capture isn't
	 * available in the WebKitGTK harness, so this injects a ready-made WAV draft
	 * via `window.__test.injectVoiceNote` instead of driving the recorder.
	 *
	 * `audioDurationMs` defaults to `durationMs`; pass a smaller value to simulate
	 * a recording whose metadata duration overshoots the real audio length.
	 */
	async recordVoiceNote(
		durationMs = 3000,
		audioDurationMs = durationMs,
	): Promise<void> {
		await this.agent.execute(
			(ms: number, ams: number) => {
				window.__test.injectVoiceNote(ms, ams);
			},
			durationMs,
			audioDurationMs,
		);
	}

	/** Paste a single synthesized PNG named `${label}.png` into the composer. */
	async pastePhotos(label: string): Promise<void> {
		await this.messageInput.waitForExist();
		await this.agent.execute(
			(pngBytes: number[], name: string) => {
				window.__test.pasteFiles([
					{ name: `${name}.png`, mimeType: 'image/png', bytes: pngBytes },
				]);
			},
			TINY_PNG_BYTES,
			label,
		);
		await this.waitForStagedMedia();
	}

	/** Drop a single synthesized PNG named `${label}.png` onto the window. */
	async dropPhotos(label: string): Promise<void> {
		await this.messageInput.waitForExist();
		await this.agent.execute(
			(pngBytes: number[], name: string) => {
				window.__test.dropFiles([
					{ name: `${name}.png`, mimeType: 'image/png', bytes: pngBytes },
				]);
			},
			TINY_PNG_BYTES,
			label,
		);
		await this.waitForStagedMedia();
	}

	/** Type `text` and send it by dispatching Enter, the way a desktop user
	 * sends. An operation arriving in the type→Enter window re-renders the
	 * composer and can swallow the keydown, so if the textarea hasn't cleared,
	 * dispatch Enter once more — the send() `sending` guard makes the retry a
	 * no-op when the first send is merely slow. */
	async sendMessage(text: string): Promise<void> {
		// In direct chats the composer only mounts once the chat leaves the
		// pending state, which depends on the peer's profile syncing
		// peer-to-peer through the mailbox.
		await this.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		await this.typeInto(tid('message-input-textarea'), text);
		await this.agent.pause(50);
		await this.dispatchEnter();
		try {
			await this.agent.waitUntil(
				async () => (await this.textareaValue()) === '',
				{ timeout: 5_000 },
			);
		} catch {
			await this.dispatchEnter();
			await this.agent.waitUntil(
				async () => (await this.textareaValue()) === '',
				{ timeoutMsg: `Composer did not clear after sending "${text}"` },
			);
		}
	}

	private async dispatchEnter(): Promise<void> {
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

	private textareaValue(): Promise<string> {
		return this.agent.execute(
			(sel: string) =>
				(document.querySelector(sel) as HTMLTextAreaElement | null)?.value ??
				'',
			tid('message-input-textarea'),
		);
	}

	/** Type `text` into the composer textarea without sending. */
	async type(text: string): Promise<void> {
		await this.agent.execute(
			(sel: string, val: string) => {
				const el = document.querySelector(sel) as HTMLTextAreaElement;
				const setter = Object.getOwnPropertyDescriptor(
					HTMLTextAreaElement.prototype,
					'value',
				)!.set!;
				setter.call(el, val);
				el.dispatchEvent(new Event('input', { bubbles: true }));
				el.dispatchEvent(new Event('change', { bubbles: true }));
			},
			tid('message-input-textarea'),
			text,
		);
	}

	/** Send the composer content. With the mobile staged-media page open, its
	 * own send button must be used (the composer's is covered by the overlay
	 * and shares the same testid). Otherwise the send button only renders on
	 * mobile user agents, so on desktop (CI) dispatch Enter on the textarea
	 * the way a desktop user sends. Composer must already have content. */
	async send(): Promise<void> {
		if (await this.stagedMediaPage.isExisting()) {
			// The staged-media page's send button (the composer's is covered by the
			// overlay and shares its testid) sits in a virtual-keyboard-composited
			// surface, so a WDA native tap misses it — click it via the DOM instead.
			await this.domClick(
				`${tid('staged-media-page')} ${tid('message-input-send')}`,
			);
			return;
		}
		if (await this.sendButton.isExisting()) {
			await this.sendButton.click();
			return;
		}
		await this.dispatchEnter();
	}

	/** Discard the staged draft: the preview's remove button on desktop,
	 * closing the staged-media page on mobile. */
	async removeDraft(): Promise<void> {
		if (await this.stagedMediaPage.isExisting()) {
			await this.closeStagedMediaPage();
			return;
		}
		await this.mediaPreview.$('button').click();
	}

	/** Discard every staged attachment: the clear-all button on desktop,
	 * closing the staged-media page on mobile. */
	async clearAll(): Promise<void> {
		if (await this.stagedMediaPage.isExisting()) {
			await this.closeStagedMediaPage();
			return;
		}
		await this.clearAttachments.click();
	}

	/** Remove the staged photo at `index`: the tile's remove button on desktop;
	 * on mobile select its thumbnail, then click its remove overlay. */
	async removeStagedPhoto(index: number): Promise<void> {
		if (await this.stagedMediaPage.isExisting()) {
			await this.agent.$(tid(`staged-media-thumb-${index}`)).click();
			await this.agent.$(tid(`staged-media-remove-${index}`)).click();
			return;
		}
		await this.removeAttachmentButton(index).click();
	}

	async hasMediaPreview(): Promise<boolean> {
		return (
			(await this.mediaPreview.isExisting()) ||
			(await this.stagedMediaPage.isExisting())
		);
	}

	/** Number of staged photos: preview thumbnails on desktop, carousel slides
	 * (page images minus thumbnail-strip images) on mobile. */
	async stagedPhotoCount(): Promise<number> {
		return this.agent.execute(
			(previewSel: string, pageSel: string, stripSel: string) => {
				const preview = document.querySelector(previewSel);
				if (preview) return preview.querySelectorAll('img').length;
				const page = document.querySelector(pageSel);
				if (!page) return 0;
				return (
					page.querySelectorAll('img').length -
					page.querySelectorAll(`${stripSel} img`).length
				);
			},
			tid('message-input-media-preview'),
			tid('staged-media-page'),
			tid('staged-media-strip'),
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
