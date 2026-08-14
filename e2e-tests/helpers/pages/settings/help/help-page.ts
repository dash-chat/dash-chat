import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class HelpPage extends TestHelper {
	back = this.el(tid('help-back'));
	contactUsLink = this.el(tid('help-contact-us'));
	startOfflineModeToggle = this.el(tid('help-start-offline-mode-switch'));
	versionItem = this.el(tid('help-version'));
	previewFeaturesToggle = this.el(tid('help-preview-features-toggle'));

	async ready() {
		await this.contactUsLink.waitForExist();
	}

	async enableOfflineMode(): Promise<void> {
		await this.startOfflineModeToggle.click();
		await this.agent.waitUntil(
			async () =>
				await this.agent.execute(
					() => window.localStorage.getItem('offline-mode-enabled') === 'true',
				),
			{
				timeoutMsg: 'offline mode was not enabled in localStorage',
			},
		);
	}
}
