import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupChatPage extends TestPage {
	page = this.agent.$(tid('group-chat-page'));
	back = this.agent.$(tid('group-chat-back'));
	infoLink = this.agent.$(tid('group-chat-info-link'));
	headerName = this.agent.$(tid('group-chat-header-name'));
	messages = this.agent.$(tid('group-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scrollBottom = this.agent.$(tid('chat-scroll-bottom'));
	unreadBadge = this.agent.$(tid('chat-unread-badge'));
	unreadDivider = this.agent.$(tid('group-chat-unread-divider'));
	scroll = new ReverseScrollPage(this.agent, 'group-chat-scroll');

	async ready() {
		await this.infoLink.waitForExist();
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

	/** True if the unread divider precedes (in DOM order) the message wrapper containing `text`. */
	async unreadDividerPrecedes(messageText: string): Promise<boolean> {
		return this.agent.execute(
			(dividerSel: string, messagesSel: string, text: string) => {
				const divider = document.querySelector(dividerSel);
				if (!divider) return false;
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(text)) {
						return !!(
							divider.compareDocumentPosition(wrapper) &
							Node.DOCUMENT_POSITION_FOLLOWING
						);
					}
				}
				return false;
			},
			tid('group-chat-unread-divider'),
			tid('group-chat-messages'),
			messageText,
		);
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
					tid('group-chat-messages'),
					text,
				),
			{ timeout, timeoutMsg: `Message "${text}" not found` },
		);
	}

	async getAuthorInitials(messageText: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${sel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const avatar = wrapper.querySelector('wa-avatar') as
							| (Element & { initials?: string })
							| null;
						return avatar?.initials || null;
					}
				}
				return null;
			},
			tid('group-chat-messages'),
			messageText,
		);
	}
}
