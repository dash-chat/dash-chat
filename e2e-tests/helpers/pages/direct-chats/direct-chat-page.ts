import { tid } from '../../selectors';
import { TestPage } from '../test-page';

const SCROLL_BOTTOM_THRESHOLD = 200;

export type MessageStatus = 'sending' | 'local' | 'cloud';
export type ConnectionStatus = 'connected' | 'local' | 'disconnected';

export class DirectChatPage extends TestPage {
	page = this.agent.$(tid('direct-chat-page'));
	back = this.agent.$(tid('direct-chat-back'));
	searchBack = this.agent.$(tid('direct-chat-search-back'));
	settingsLink = this.agent.$(tid('direct-chat-settings-link'));
	peerName = this.agent.$(tid('direct-chat-peer-name'));
	peerHeader = this.agent.$(tid('direct-chat-peer-header'));
	scroll = this.agent.$(tid('direct-chat-scroll'));
	scrollBottom = this.agent.$(tid('direct-chat-scroll-bottom'));
	unreadBadge = this.agent.$(tid('direct-chat-unread-badge'));
	unreadDivider = this.agent.$(tid('direct-chat-unread-divider'));
	acceptButton = this.agent.$(tid('direct-chat-accept-btn'));
	rejectButton = this.agent.$(tid('direct-chat-reject-btn'));
	acceptConfirm = this.agent.$(tid('direct-chat-accept-confirm'));
	rejectConfirm = this.agent.$(tid('direct-chat-reject-confirm'));
	messages = this.agent.$(tid('direct-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	sendButton = this.agent.$(tid('message-input-send'));
	emojiButton = this.agent.$(tid('message-input-emoji'));
	messageStatus = this.agent.$(tid('message-status'));
	connectionStatusIndicator = this.agent.$(tid('connection-status'));

	async ready() {
		await this.page.waitForExist();
	}

	async sendMessage(text: string) {
		await this.typeInto(tid('message-input-textarea'), text);
		await this.agent.pause(50);
		await this.agent.execute((sel: string) => {
			(document.querySelector(sel) as HTMLElement).click();
		}, tid('message-input-send'));
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

	/** Read the navbar connection chip. Absence === 'connected'. */
	async connectionStatus(): Promise<ConnectionStatus> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return 'connected';
			const status = el.dataset.status;
			if (status === 'local' || status === 'disconnected') return status;
			throw new Error(`connectionStatus: unexpected data-status="${status}"`);
		}, tid('connection-status'));
	}

	async isScrollAtBottom(): Promise<boolean> {
		return this.agent.execute(
			(sel: string, threshold: number) => {
				const el = document.querySelector(sel) as HTMLElement | null;
				if (!el) throw new Error('isScrollAtBottom: scroll container not found');
				return Math.abs(el.scrollTop) < threshold;
			},
			tid('direct-chat-scroll'),
			SCROLL_BOTTOM_THRESHOLD,
		);
	}

	async chatOverflow(): Promise<number> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return 0;
			return el.scrollHeight - el.clientHeight;
		}, tid('direct-chat-scroll'));
	}

	async scrollChatUp(): Promise<void> {
		await this.agent.execute(
			(sel: string, threshold: number) => {
				const el = document.querySelector(sel) as HTMLElement | null;
				if (!el) throw new Error('scrollChatUp: scroll container not found');
				const max = el.scrollHeight - el.clientHeight;
				if (max <= threshold) {
					throw new Error(
						`scrollChatUp: not enough overflow (max=${max}); send more messages first`,
					);
				}
				const distance = Math.min(max, 600);
				el.scrollTop = -distance;
				if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
				el.dispatchEvent(new Event('scroll'));
			},
			tid('direct-chat-scroll'),
			SCROLL_BOTTOM_THRESHOLD,
		);
	}

	async scrollChatToBottom(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) throw new Error('scrollChatToBottom: scroll container not found');
			el.scrollTop = 0;
			el.dispatchEvent(new Event('scroll'));
		}, tid('direct-chat-scroll'));
	}

	async scrollChatToTop(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) throw new Error('scrollChatToTop: scroll container not found');
			const distance = el.scrollHeight - el.clientHeight;
			el.scrollTop = -distance;
			if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
			el.dispatchEvent(new Event('scroll'));
		}, tid('direct-chat-scroll'));
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

	/** Inline opacity of the transparent navbar bg element. */
	async navbarBgOpacity(): Promise<string | null> {
		return this.agent.execute(() => {
			const candidates = document.querySelectorAll('.k-navbar > div.absolute');
			const bg = candidates[candidates.length - 1] as HTMLElement | undefined;
			return bg?.style.opacity ?? null;
		});
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
}
