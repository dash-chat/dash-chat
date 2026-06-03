import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupChatPage extends TestPage {
	page = this.agent.$(tid('group-chat-page'));
	back = this.agent.$(tid('group-chat-back'));
	infoLink = this.agent.$(tid('group-chat-info-link'));
	messages = this.agent.$(tid('group-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	sendButton = this.agent.$(tid('message-input-send'));
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

	async sendMessage(text: string) {
		await this.typeInto(tid('message-input-textarea'), text);
		await this.agent.pause(50);
		await this.sendButton.click();
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
						const avatar = wrapper.querySelector('wa-avatar');
						if (!avatar) return null;
						const attr = avatar.getAttribute('initials');
						const prop = (avatar as unknown as { initials?: string }).initials;
						return attr || prop || null;
					}
				}
				return null;
			},
			tid('group-chat-messages'),
			messageText,
		);
	}

	async getMessageHash(messageText: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${sel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						return wrapper.getAttribute('data-message-hash');
					}
				}
				return null;
			},
			tid('group-chat-messages'),
			messageText,
		);
	}

	async getSenderName(hash: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, h: string) => {
				const wrapper = document.querySelector<HTMLElement>(
					`${sel} [data-message-hash="${h}"]`,
				);
				const name = wrapper?.querySelector(
					'[data-testid="group-message-sender-name"]',
				);
				return name?.textContent?.trim() ?? null;
			},
			tid('group-chat-messages'),
			hash,
		);
	}

	async getSenderColorVar(hash: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, h: string) => {
				const wrapper = document.querySelector<HTMLElement>(
					`${sel} [data-message-hash="${h}"]`,
				);
				const name = wrapper?.querySelector(
					'[data-testid="group-message-sender-name"]',
				);
				const style = name?.getAttribute('style') ?? '';
				const match = style.match(/--sender-color-\d+/);
				return match ? match[0] : null;
			},
			tid('group-chat-messages'),
			hash,
		);
	}
}
