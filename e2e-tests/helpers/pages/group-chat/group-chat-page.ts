import { Composer } from '../../components/composer';
import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { Messages } from '../../components/messages';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupChatPage extends TestPage {
	page = this.agent.$(tid('group-chat-page'));
	back = this.agent.$(tid('group-chat-back'));
	infoLink = this.agent.$(tid('group-chat-info-link'));
	headerName = this.agent.$(tid('group-chat-header-name'));
	messages = new Messages(
		this.agent,
		'group-chat-messages',
		'group-chat-unread-divider',
	);
	composer = new Composer(this.agent);
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scroll = new ReverseScrollPage(this.agent, 'group-chat-scroll');

	async ready() {
		await this.infoLink.waitForExist();
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

	systemMessage(
		kind:
			| 'group_created'
			| 'group_member_added'
			| 'group_member_removed'
			| 'group_member_promoted'
			| 'group_member_demoted',
	) {
		return this.agent.$(tid(`group-chat-system-message-${kind}`));
	}
}
