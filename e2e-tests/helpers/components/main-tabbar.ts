import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class MainTabbar extends TestHelper {
	chatsTab = this.el(tid('main-tab-chats'));
	settingsTab = this.el(tid('main-tab-settings'));
	settingsHubWarning = this.el(
		`${tid('main-tab-settings')} ${tid('warning-badge')}`,
	);
}
