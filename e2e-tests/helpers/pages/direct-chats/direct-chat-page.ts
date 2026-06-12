import {
	Composer,
	waitForFileMessageIn,
	waitForPhotoMessageIn,
} from '../../components/composer';
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
	composer = new Composer(this.agent);
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
	 * Attach `count` photos (synthesized 1×1 PNGs) to the composer. See
	 * `Composer.attachPhotos`.
	 */
	async attachPhotos(count = 1): Promise<void> {
		await this.composer.attachPhotos(count);
	}

	/** Attach a single non-image file to the composer. */
	async attachFile(
		name = 'notes.txt',
		contents = 'hello from e2e',
		mimeType = 'text/plain',
	): Promise<void> {
		await this.composer.attachFile(name, contents, mimeType);
	}

	/** Attach a zero-filled file of exactly `sizeBytes` to test the size cap. */
	async attachFileOfSize(sizeBytes: number, name = 'big.bin'): Promise<void> {
		await this.composer.attachFileOfSize(sizeBytes, name);
	}

	/** Click send. Composer must already have content (text and/or media). */
	async sendComposer(): Promise<void> {
		await this.composer.send();
	}

	/** Remove the currently-attached draft via the preview's remove button. */
	async removeDraft(): Promise<void> {
		await this.composer.removeDraft();
	}

	async hasMediaPreview(): Promise<boolean> {
		return this.composer.hasMediaPreview();
	}

	/** Number of photo thumbnails currently staged in the composer preview. */
	async stagedPhotoCount(): Promise<number> {
		return this.composer.stagedPhotoCount();
	}

	/** Wait until exactly `expected` photos are staged in the preview. */
	async expectStagedPhotoCount(expected: number): Promise<void> {
		await this.composer.expectStagedPhotoCount(expected);
	}

	/** First clickable photo cell of the first photo message in the chat. */
	photoCellButton() {
		return this.messages.$(`${tid('message-attachment-photos')} button`);
	}

	/** Wait until a rendered (loaded) photo attachment appears in the chat. */
	async waitForPhotoMessage(timeout = 25_000): Promise<void> {
		await waitForPhotoMessageIn(
			this.agent,
			tid('direct-chat-messages'),
			timeout,
		);
	}

	/** Wait until a file attachment with the given filename appears. */
	async waitForFileMessage(name: string, timeout = 25_000): Promise<void> {
		await waitForFileMessageIn(
			this.agent,
			tid('direct-chat-messages'),
			name,
			timeout,
		);
	}
}
