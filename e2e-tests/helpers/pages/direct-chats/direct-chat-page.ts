import { Composer } from '../../components/composer';
import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { type MessageStatus, Messages } from '../../components/messages';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { SYNC_TIMEOUT } from '../../timeouts';
import { TestHelper } from '../test-helper';

export type { MessageStatus } from '../../components/messages';
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
	peerAvatar = this.el(tid('direct-chat-peer-avatar'));
	nameNotVerifiedPill = this.el(tid('direct-chat-name-not-verified'));
	acceptButton = this.el(tid('direct-chat-accept-btn'));
	acceptConfirm = this.el(tid('direct-chat-accept-confirm'));
	blockButton = this.el(tid('direct-chat-block-btn'));
	reportButton = this.el(tid('direct-chat-report-btn'));
	unblockButton = this.el(tid('direct-chat-unblock-btn'));
	blockedBanner = this.el(tid('direct-chat-blocked-banner'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	reportConfirm = this.el(tid('report-contact-confirm'));
	unblockConfirm = this.el(tid('unblock-contact-confirm'));
	blockedNameIcon = this.el(tid('blocked-name-icon'));
	reportMessage = this.el(tid('direct-chat-report-message'));
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
	lastMessageStatus(): Promise<MessageStatus | null> {
		return this.messages.lastMessageStatus();
	}

	/** Read the data-status of the status indicator inside the bubble whose text contains `text`. */
	messageStatusFor(text: string): Promise<MessageStatus | null> {
		return this.messages.messageStatusFor(text);
	}

	/** How many report bubbles the chat currently shows. */
	async reportMessageCount(): Promise<number> {
		const bubbles = await this.agent.$$(tid('direct-chat-report-message'));
		return bubbles.length;
	}

	isPeerNamePresent(): Promise<boolean> {
		return this.peerName.isExisting();
	}

	/**
	 * Wait until the peer's real profile is rendered: the chat has left the
	 * pending state and the avatar is no longer the placeholder. Gate on this —
	 * not on the peer name, which also renders for a pending chat from the name
	 * carried in the scanned code — whenever a test needs an established
	 * contact.
	 */
	async waitForPeerProfile(): Promise<void> {
		// peerAvatarIsPlaceholder() reads an attribute off the avatar, so it
		// answers "not a placeholder" while the header is still unmounted. Wait
		// for the element first or the check below passes vacuously.
		await this.peerAvatar.waitForExist();
		await this.agent.waitUntil(
			async () => !(await this.peerAvatarIsPlaceholder()),
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg: "Peer's profile did not arrive",
			},
		);
	}

	/** Whether the peer-header avatar is the profile placeholder (person icon,
	 * shown while the full profile is still syncing) rather than a real one. */
	peerAvatarIsPlaceholder(): Promise<boolean> {
		return this.agent.execute(
			(sel: string) =>
				document.querySelector(sel)?.getAttribute('data-waiting') === 'true',
			tid('direct-chat-peer-avatar'),
		);
	}

	isContactRequestBannerVisible(): Promise<boolean> {
		return this.acceptButton.isExisting();
	}

	/** Accept an incoming contact request (open the confirm dialog, confirm).
	 * Uses DOM clicks: a WDA native tap on these Konsta buttons doesn't reliably
	 * fire on iOS (same limitation as the composer send button). */
	async acceptContactRequest(): Promise<void> {
		await this.domClick(tid('direct-chat-accept-btn'));
		await this.domClick(tid('direct-chat-accept-confirm'));
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
