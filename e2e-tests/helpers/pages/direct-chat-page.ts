import { tid } from '../../../ui/tests/selectors';
import { TestPage } from './test-page';

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

	async ready() {
		await this.page.waitForExist();
	}

	async sendMessage(text: string) {
		const selector = tid('message-input-textarea');
		await this.agent.$(selector).waitForExist();
		await this.agent.execute(
			(sel: string, value: string) => {
				const el = document.querySelector(sel) as HTMLTextAreaElement;
				const setter = Object.getOwnPropertyDescriptor(
					HTMLTextAreaElement.prototype,
					'value',
				)!.set!;
				setter.call(el, value);
				el.dispatchEvent(new Event('input', { bubbles: true }));
				el.dispatchEvent(new Event('change', { bubbles: true }));
			},
			selector,
			text,
		);
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
}
