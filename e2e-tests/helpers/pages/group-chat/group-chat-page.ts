import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupChatPage extends TestPage {
	page = this.agent.$(tid('group-chat-page'));
	back = this.agent.$(tid('group-chat-back'));
	infoLink = this.agent.$(tid('group-chat-info-link'));
	messages = this.agent.$(tid('group-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	sendButton = this.agent.$(tid('message-input-send'));
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);

	async ready() {
		await this.infoLink.waitForExist();
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
}
