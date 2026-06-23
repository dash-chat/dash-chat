import { TestPage } from '../test-page';

export class SettingsPage extends TestPage {
	back = this.el('settings-back');
	profileLink = this.el('settings-profile-link');
	qrLink = this.el('settings-qr-link');
	appearanceLink = this.el('settings-appearance-link');
	accountLink = this.el('settings-account-link');
	helpLink = this.el('settings-help-link');
	notificationsLink = this.el('settings-notifications-link');
	offlineLink = this.el('settings-offline-link');

	async ready() {
		await this.profileLink.waitForExist();
	}
}
