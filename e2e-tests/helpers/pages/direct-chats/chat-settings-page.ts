import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class ChatSettingsPage extends TestHelper {
	back = this.el(tid('chat-settings-back'));
	peerName = this.el(tid('chat-settings-peer-name'));
	peerHeader = this.el(tid('chat-settings-peer-header'));
	searchButton = this.el(tid('chat-settings-search-btn'));
	blockButton = this.el(tid('chat-settings-block-btn'));
	unblockButton = this.el(tid('chat-settings-unblock-btn'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	unblockConfirm = this.el(tid('unblock-contact-confirm'));

	async ready() {
		await this.peerHeader.waitForExist();
	}
}
