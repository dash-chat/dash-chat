import { tid } from '../../selectors';
import { TestPage } from '../test-page';

const TOGGLE_INPUT_SELECTOR = `${tid('offline-local-mailbox-toggle')} input[type="checkbox"]`;

export class OfflinePage extends TestPage {
	back = this.el('offline-back');
	localMailboxToggle = this.el('offline-local-mailbox-toggle');
	toggleInput = this.agent.$(TOGGLE_INPUT_SELECTOR);

	async ready() {
		await this.localMailboxToggle.waitForExist();
	}

	private async toggleChecked(): Promise<boolean> {
		return this.agent.execute(
			(sel: string) =>
				(document.querySelector(sel) as HTMLInputElement | null)?.checked ??
				false,
			TOGGLE_INPUT_SELECTOR,
		);
	}

	/** Toggle the local mailbox switch. Assumes the agent is on /settings/offline. */
	async setLocalMailboxEnabled(enabled: boolean): Promise<void> {
		await this.toggleInput.waitForExist();
		if ((await this.toggleChecked()) === enabled) return;
		// The checkbox has `sr-only` styling (visually hidden), which trips
		// WebDriver's interactability check. Dispatch the click in-page instead.
		await this.agent.execute((sel: string) => {
			(document.querySelector(sel) as HTMLInputElement | null)?.click();
		}, TOGGLE_INPUT_SELECTOR);
		await this.agent.waitUntil(
			async () => (await this.toggleChecked()) === enabled,
		);
	}
}
