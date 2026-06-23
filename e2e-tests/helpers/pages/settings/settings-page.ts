import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class SettingsPage extends TestHelper {
	back = this.el(tid('settings-back'));
	profileLink = this.el(tid('settings-profile-link'));
	qrLink = this.el(tid('settings-qr-link'));
	appearanceLink = this.el(tid('settings-appearance-link'));
	accountLink = this.el(tid('settings-account-link'));
	helpLink = this.el(tid('settings-help-link'));
	notificationsLink = this.el(tid('settings-notifications-link'));
	offlineLink = this.el(tid('settings-offline-link'));

	async ready() {
		await this.profileLink.waitForExist();
	}
}
