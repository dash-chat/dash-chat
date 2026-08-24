import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

const TOGGLE_INPUT_SELECTOR = `${tid('offline-local-mailbox-toggle')} input[type="checkbox"]`;
const BACKGROUND_MODE_TOGGLE_INPUT_SELECTOR = `${tid('offline-background-mode-toggle')} input[type="checkbox"]`;

export class OfflinePage extends TestHelper {
	back = this.el(tid('offline-back'));
	localMailboxToggle = this.el(tid('offline-local-mailbox-toggle'));
	toggleInput = this.el(TOGGLE_INPUT_SELECTOR);
	backgroundModeToggle = this.el(tid('offline-background-mode-toggle'));
	backgroundModeToggleInput = this.el(BACKGROUND_MODE_TOGGLE_INPUT_SELECTOR);

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

	private async checkedFor(sel: string): Promise<boolean> {
		return this.agent.execute(
			(s: string) =>
				(document.querySelector(s) as HTMLInputElement | null)?.checked ??
				false,
			sel,
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

	/**
	 * Toggle the Android background mode switch. Assumes the agent is on
	 * /settings/offline and running on Android (the toggle only renders there).
	 */
	async setBackgroundModeEnabled(enabled: boolean): Promise<void> {
		await this.backgroundModeToggleInput.waitForExist();
		if (
			(await this.checkedFor(BACKGROUND_MODE_TOGGLE_INPUT_SELECTOR)) === enabled
		)
			return;
		await this.agent.execute((sel: string) => {
			(document.querySelector(sel) as HTMLInputElement | null)?.click();
		}, BACKGROUND_MODE_TOGGLE_INPUT_SELECTOR);
		await this.agent.waitUntil(
			async () =>
				(await this.checkedFor(BACKGROUND_MODE_TOGGLE_INPUT_SELECTOR)) ===
				enabled,
		);
	}
}
