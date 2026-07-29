import { Composer } from '../../components/composer';
import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { Messages } from '../../components/messages';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export type MessageStatus = 'sending' | 'local' | 'cloud';
export type { ConnectionStatus } from '../../components/connection-status-indicator';

export class DirectChatPage extends TestHelper {
	page = this.el(tid('direct-chat-page'));
	back = this.el(tid('direct-chat-back'));
	searchBack = this.el(tid('direct-chat-search-back'));
	searchInput = this.el(tid('direct-chat-search-input'));
	searchResultsCount = this.el(tid('search-results-count'));
	settingsLink = this.el(tid('direct-chat-settings-link'));
	peerName = this.el(tid('direct-chat-peer-name'));
	peerHeader = this.el(tid('direct-chat-peer-header'));
	acceptButton = this.el(tid('direct-chat-accept-btn'));
	rejectButton = this.el(tid('direct-chat-reject-btn'));
	acceptConfirm = this.el(tid('direct-chat-accept-confirm'));
	rejectConfirm = this.el(tid('direct-chat-reject-confirm'));
	blockButton = this.el(tid('direct-chat-block-btn'));
	unblockButton = this.el(tid('direct-chat-unblock-btn'));
	blockedBanner = this.el(tid('direct-chat-blocked-banner'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	reportButton = this.el(tid('direct-chat-report-btn'));
	reportConfirm = this.el(tid('report-contact-confirm'));
	blockedNameIcon = this.el(tid('blocked-name-icon'));
	messageStatus = this.el(tid('message-status'));
	readMore = this.el(tid('message-read-more'));
	composer = new Composer(this.agent);
	messages = new Messages(
		this.agent,
		'direct-chat-messages',
		'direct-chat-unread-divider',
		this.composer,
	);
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scroll = new ReverseScrollPage(this.agent, 'direct-chat-scroll');

	async ready() {
		await this.page.waitForExist();
	}

	async searchFor(query: string) {
		await this.searchInput.waitForExist();
		await this.typeInto(tid('direct-chat-search-input'), query);
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

	/** Read the data-status of the status indicator inside the bubble whose text contains `text`. */
	async messageStatusFor(text: string): Promise<MessageStatus | null> {
		return this.agent.execute(
			(messagesSel: string, statusSel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (!wrapper.textContent?.includes(t)) continue;
					const el = wrapper.querySelector(statusSel) as HTMLElement | null;
					const status = el?.dataset.status;
					if (
						status === 'sending' ||
						status === 'local' ||
						status === 'cloud'
					) {
						return status;
					}
					return null;
				}
				return null;
			},
			tid('direct-chat-messages'),
			tid('message-status'),
			text,
		);
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
				const style = window.getComputedStyle(el);
				const clipped =
					style.overflowX === 'hidden' ||
					style.overflowX === 'clip' ||
					style.overflow === 'hidden' ||
					style.overflow === 'clip' ||
					style.textOverflow === 'ellipsis';
				if (clipped) return;
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
