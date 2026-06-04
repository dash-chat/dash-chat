import { tid } from '../selectors';

export type UpdateState =
	| 'available'
	| 'downloading'
	| 'ready'
	| 'error'
	| 'hidden';

export class UpdaterBanner {
	constructor(private agent: WebdriverIO.Browser) {}

	banner = this.agent.$(tid('updater-banner'));
	title = this.agent.$(tid('updater-banner-title'));
	dismissButton = this.agent.$(tid('updater-dismiss-btn'));

	isVisible(): Promise<boolean> {
		return this.banner.isExisting();
	}

	/** Trigger the banner into a specific state via window.__test. */
	async simulateUpdate(state: UpdateState): Promise<void> {
		await this.agent.execute(
			(s: UpdateState) => window.__test.simulateUpdate(s),
			state,
		);
	}

	async dismiss(): Promise<void> {
		await this.dismissButton.click();
	}
}
