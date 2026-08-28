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

	/** Tap the version row `times` in a row. Driven from inside the page so every
	 * gap is well under the 300ms the developer-mode unlock allows between taps —
	 * one WDIO click per tap would spend longer than that in round trips. */
	async tapVersion(times: number): Promise<void> {
		await this.versionItem.waitForExist();
		await this.agent.execute(
			(sel: string, count: number) => {
				const el = document.querySelector(sel) as HTMLElement;
				for (let i = 0; i < count; i++) el.click();
			},
			tid('help-version'),
			times,
		);
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
