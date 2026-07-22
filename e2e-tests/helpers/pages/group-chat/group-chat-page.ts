import { Composer } from '../../components/composer';
import { ConnectionStatusIndicator } from '../../components/connection-status-indicator';
import { Messages } from '../../components/messages';
import { ReverseScrollPage } from '../../components/reverse-scroll-page';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class GroupChatPage extends TestHelper {
	page = this.el(tid('group-chat-page'));
	back = this.el(tid('group-chat-back'));
	infoLink = this.el(tid('group-chat-info-link'));
	headerName = this.el(tid('group-chat-header-name'));
	messages = new Messages(
		this.agent,
		'group-chat-messages',
		'group-chat-unread-divider',
	);
	composer = new Composer(this.agent);
	notMemberNotice = this.el(tid('group-chat-not-member'));
	connectionStatusIndicator = new ConnectionStatusIndicator(this.agent);
	scroll = new ReverseScrollPage(this.agent, 'group-chat-scroll');

	async ready() {
		await this.infoLink.waitForExist();
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
