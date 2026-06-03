import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class SettingsPage extends TestPage {
	back = this.agent.$(tid('settings-back'));
	profileLink = this.agent.$(tid('settings-profile-link'));
	qrLink = this.agent.$(tid('settings-qr-link'));
	appearanceLink = this.agent.$(tid('settings-appearance-link'));
	accountLink = this.agent.$(tid('settings-account-link'));
	helpLink = this.agent.$(tid('settings-help-link'));
	notificationsLink = this.agent.$(tid('settings-notifications-link'));
	offlineLink = this.agent.$(tid('settings-offline-link'));

	async ready() {
		await this.profileLink.waitForExist();
	}
}
