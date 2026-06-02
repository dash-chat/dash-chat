import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupChatPage extends TestPage {
	back = this.agent.$(tid('group-chat-back'));
	infoLink = this.agent.$(tid('group-chat-info-link'));
	messages = this.agent.$(tid('direct-chat-messages'));
	messageInput = this.agent.$(tid('message-input-textarea'));
	sendButton = this.agent.$(tid('message-input-send'));

	async ready() {
		await this.infoLink.waitForExist();
	}

	async sendMessage(text: string) {
		await this.typeInto(tid('message-input-textarea'), text);
		await this.agent.pause(50);
		await this.sendButton.click();
	}
}
