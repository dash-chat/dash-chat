import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class ChatSettingsPage extends TestPage {
	back = this.agent.$(tid('chat-settings-back'));
	peerName = this.agent.$(tid('chat-settings-peer-name'));
	peerHeader = this.agent.$(tid('chat-settings-peer-header'));
	searchButton = this.agent.$(tid('chat-settings-search-btn'));

	async ready() {
		await this.peerHeader.waitForExist();
	}
}
