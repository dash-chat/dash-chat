import { Composer } from '../../components/composer';
import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { Messages } from '../../components/messages';
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
	acceptButton = this.agent.$(tid('direct-chat-accept-btn'));
	rejectButton = this.agent.$(tid('direct-chat-reject-btn'));
	acceptConfirm = this.agent.$(tid('direct-chat-accept-confirm'));
	rejectConfirm = this.agent.$(tid('direct-chat-reject-confirm'));
	messageStatus = this.agent.$(tid('message-status'));
	messages = new Messages(
		this.agent,
		'direct-chat-messages',
		'direct-chat-unread-divider',
	);
	composer = new Composer(this.agent);
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scroll = new ReverseScrollPage(this.agent, 'direct-chat-scroll');
	quickEditButton = this.agent.$(tid('quick-edit-button'));
	editHistorySheet = this.agent.$(tid('edit-history-sheet'));

	async ready() {
		await this.page.waitForExist();
	}

	/** Long-press (contextmenu) the message with `oldText`, tap Edit, replace the
	 * text with `newText`, and send. */
	async editMessage(oldText: string, newText: string): Promise<void> {
		await this.messages.openActions(oldText);
		await this.quickEditButton.waitForClickable();
		await this.quickEditButton.click();
		await this.composer.editingBanner.waitForExist();
		await this.composer.type(newText);
		await this.composer.send();
	}

	/** Text of each version listed in the open edit-history sheet, newest first. */
	async editHistoryVersions(): Promise<string[]> {
		await this.editHistorySheet.waitForExist();
		return this.agent.execute((sel: string) => {
			const sheet = document.querySelector(sel);
			if (!sheet) return [];
			return Array.from(sheet.querySelectorAll('.whitespace-pre-wrap')).map(
				el => el.textContent?.trim() ?? '',
			);
		}, tid('edit-history-sheet'));
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
