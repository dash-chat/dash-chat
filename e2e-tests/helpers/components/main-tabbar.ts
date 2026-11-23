import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class MainTabbar extends TestHelper {
	desktopNavStrip = this.el(tid('desktop-nav-strip'));
	chatsTab = this.el(tid('main-tab-chats'));
	chatsUnreadBadge = this.el(tid('chats-unread-badge'));
	settingsTab = this.el(tid('main-tab-settings'));
	settingsHubWarning = this.el(
		`${tid('main-tab-settings')} ${tid('warning-badge')}`,
	);
}
