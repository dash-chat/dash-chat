import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class NotificationsPage extends TestHelper {
	back = this.el(tid('notifications-back'));
	toggle = this.el(tid('notifications-toggle'));
	toggleInput = this.el(`${tid('notifications-toggle')} input`);
	permissionBanner = this.el(tid('notifications-permission-banner'));
	turnOnButton = this.el(tid('notifications-permission-turn-on'));
	settingsSheet = this.el(tid('permission-settings-sheet'));

	async ready() {
		await this.toggle.waitForExist();
	}
}
