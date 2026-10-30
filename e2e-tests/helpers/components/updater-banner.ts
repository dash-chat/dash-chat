import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export type UpdateState =
	| 'available'
	| 'downloading'
	| 'ready'
	| 'error'
	| 'hidden';

export class UpdaterBanner extends TestHelper {
	banner = this.el(tid('updater-banner'));
	title = this.el(tid('updater-banner-title'));
	dismissButton = this.el(tid('updater-dismiss-btn'));

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

	/** Run the banner's real update check (plugin check → banner), which
	 *  mounting skips in e2e builds (their frontend is DEV-flagged). */
	async checkForUpdate(): Promise<void> {
		await this.agent.execute(() => window.__test.simulateUpdate('check'));
	}

	async dismiss(): Promise<void> {
		await this.dismissButton.click();
	}
}
