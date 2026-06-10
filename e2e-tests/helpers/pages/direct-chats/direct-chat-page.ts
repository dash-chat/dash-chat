import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export type MessageStatus = 'sending' | 'local' | 'cloud';
export type { ConnectionStatus } from '../../components/connection-status-indicator';

export class DirectChatPage extends TestPage {
	page = this.agent.$(tid('direct-chat-page'));
	back = this.agent.$(tid('direct-chat-back'));
	searchBack = this.agent.$(tid('direct-chat-search-back'));
	settingsLink = this.agent.$(tid('direct-chat-settings-link'));
	peerName = this.agent.$(tid('direct-chat-peer-name'));
	peerHeader = this.agent.$(tid('direct-chat-peer-header'));
	scrollBottom = this.agent.$(tid('chat-scroll-bottom'));
	unreadBadge = this.agent.$(tid('chat-unread-badge'));
	unreadDivider = this.agent.$(tid('direct-chat-unread-divider'));
	acceptButton = this.agent.$(tid('direct-chat-accept-btn'));
	rejectButton = this.agent.$(tid('direct-chat-reject-btn'));
	acceptConfirm = this.agent.$(tid('direct-chat-accept-confirm'));
	rejectConfirm = this.agent.$(tid('direct-chat-reject-confirm'));
	messages = this.agent.$(tid('direct-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	emojiButton = this.agent.$(tid('message-input-emoji'));
	messageStatus = this.agent.$(tid('message-status'));
	sendButton = this.agent.$(tid('message-input-send'));
	mediaPreview = this.agent.$(tid('message-input-media-preview'));
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scroll = new ReverseScrollPage(this.agent, 'direct-chat-scroll');

	async ready() {
		await this.page.waitForExist();
	}

	async sendMessage(text: string) {
		await this.typeInto(tid('message-input-textarea'), text);
		await this.agent.pause(50);
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

	async waitForMessage(text: string, timeout = 25_000) {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(sel: string, t: string) =>
						document.querySelector(sel)?.textContent?.includes(t) ?? false,
					tid('direct-chat-messages'),
					text,
				),
			{ timeout, timeoutMsg: `Message "${text}" not found` },
		);
	}

	/** Read the data-status of the most recent message-status indicator. */
	async lastMessageStatus(): Promise<MessageStatus | null> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			const status = el?.dataset.status;
			if (status === 'sending' || status === 'local' || status === 'cloud') {
				return status;
			}
			return null;
		}, tid('message-status'));
	}

	scrollBottomButtonVisible(): Promise<boolean> {
		return this.scrollBottom.isExisting();
	}

	async unreadBadgeText(): Promise<string | null> {
		if (!(await this.unreadBadge.isExisting())) return null;
		const text = (await this.unreadBadge.getText()).trim();
		return text === '' ? null : text;
	}

	async clickScrollBottomButton(): Promise<void> {
		await this.scrollBottom.click();
	}

	isPeerNamePresent(): Promise<boolean> {
		return this.peerName.isExisting();
	}

	isContactRequestBannerVisible(): Promise<boolean> {
		return this.acceptButton.isExisting();
	}

	/** Returns descriptions of any direct-chat navbar overflow issues. */
	checkNavbarOverflow(): Promise<string[]> {
		return this.agent.execute(() => {
			const navbar = document.querySelector('.k-navbar');
			if (!navbar) return ['Navbar element not found'];
			const issues: string[] = [];
			if (navbar.scrollWidth > navbar.clientWidth + 2) {
				issues.push('Navbar has horizontal overflow');
			}
			navbar.querySelectorAll('*').forEach(el => {
				if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
					const text = el.textContent?.substring(0, 60).trim();
					if (text)
						issues.push(
							`Overflow in navbar <${el.tagName.toLowerCase()}>: "${text}"`,
						);
				}
			});
			return issues.slice(0, 10);
		});
	}

	/**
	 * Attach `count` photos (synthesized 1×1 PNGs) to the composer. The hidden
	 * file input is populated via DataTransfer + a synthetic change event, the
	 * same trick add-contact uses for QR uploads.
	 */
	async attachPhotos(count = 1): Promise<void> {
		await this.agent.execute((n: number) => {
			const TINY_PNG = new Uint8Array([
				0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
				0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
				0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00,
				0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
				0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
				0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
			]);
			const input = document.querySelector(
				'[data-testid="message-input-photo-picker"]',
			) as HTMLInputElement;
			const dt = new DataTransfer();
			for (let i = 1; i <= n; i++) {
				const blob = new Blob([TINY_PNG], { type: 'image/png' });
				dt.items.add(new File([blob], `photo-${i}.png`, { type: 'image/png' }));
			}
			input.files = dt.files;
			input.dispatchEvent(new Event('change', { bubbles: true }));
		}, count);
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

	/** Click send. Composer must already have content (text and/or media). */
	async sendComposer(): Promise<void> {
		await this.sendButton.click();
	}

	/** Remove the currently-attached draft via the preview's remove button. */
	async removeDraft(): Promise<void> {
		await this.mediaPreview.$('button').click();
	}

	async hasMediaPreview(): Promise<boolean> {
		return this.mediaPreview.isExisting();
	}

	/** Wait until a rendered (loaded) photo attachment appears in the chat. */
	async waitForPhotoMessage(timeout = 25_000): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(messagesSel: string, photosSel: string) => {
						const img = document
							.querySelector(messagesSel)
							?.querySelector(`${photosSel} img`) as HTMLImageElement | null;
						return !!img && img.complete && img.naturalWidth > 0;
					},
					tid('direct-chat-messages'),
					tid('message-attachment-photos'),
				),
			{ timeout, timeoutMsg: 'Photo message not found' },
		);
	}

	/** Wait until a file attachment with the given filename appears. */
	async waitForFileMessage(name: string, timeout = 25_000): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(messagesSel: string, fileSel: string, n: string) => {
						const root = document.querySelector(messagesSel);
						const files = root?.querySelectorAll(fileSel) ?? [];
						return Array.from(files).some(f => f.textContent?.includes(n));
					},
					tid('direct-chat-messages'),
					tid('message-attachment-file'),
					name,
				),
			{ timeout, timeoutMsg: `File message "${name}" not found` },
		);
	}
}
