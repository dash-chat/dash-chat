import { TestPage } from '../test-page';

export class ChatSettingsPage extends TestPage {
	back = this.el('chat-settings-back');
	peerName = this.el('chat-settings-peer-name');
	peerHeader = this.el('chat-settings-peer-header');
	searchButton = this.el('chat-settings-search-btn');

	async ready() {
		await this.peerHeader.waitForExist();
	}
}
