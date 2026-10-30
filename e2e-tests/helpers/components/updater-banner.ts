import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The banner states `window.__test.simulateUpdate` can force ('check', which
 *  runs the real update check instead, goes through [`UpdaterBanner.checkForUpdate`]). */
export type UpdateState = Exclude<
	Parameters<Window['__test']['simulateUpdate']>[0],
	'check'
>;

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

	/** Run the banner's real update check (plugin check → banner). Mounting
	 *  skips it in e2e builds: wdio's CLI exports NODE_ENV=test, so the
	 *  harness's frontend build compiles with import.meta.env.DEV set. */
	async checkForUpdate(): Promise<void> {
		await this.agent.execute(() => window.__test.simulateUpdate('check'));
	}

	async dismiss(): Promise<void> {
		await this.dismissButton.click();
	}
}
