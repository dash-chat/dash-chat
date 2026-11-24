import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

const LAN_ROUTER_TOGGLE_INPUT_SELECTOR = `${tid('developer-lan-router-toggle')} input[type="checkbox"]`;

export class DeveloperPage extends TestHelper {
	back = this.el(tid('developer-back'));
	disable = this.el(tid('developer-disable'));
	lanRouterToggleInput = this.el(LAN_ROUTER_TOGGLE_INPUT_SELECTOR);

	async ready() {
		await this.disable.waitForExist();
	}

	async lanRouterEnabled(): Promise<boolean> {
		await this.lanRouterToggleInput.waitForExist();
		return this.agent.execute(
			(sel: string) =>
				(document.querySelector(sel) as HTMLInputElement | null)?.checked ??
				false,
			LAN_ROUTER_TOGGLE_INPUT_SELECTOR,
		);
	}

	/** Toggle the Dash Router switch and wait for the node it rebuilds. */
	async setLanRouterEnabled(enabled: boolean): Promise<void> {
		if ((await this.lanRouterEnabled()) === enabled) return;
		// The checkbox is visually hidden, which trips WebDriver's
		// interactability check.
		await this.agent.execute((sel: string) => {
			(document.querySelector(sel) as HTMLInputElement | null)?.click();
		}, LAN_ROUTER_TOGGLE_INPUT_SELECTOR);
		// The switch stays disabled until the node is rebuilt.
		await this.agent.waitUntil(
			async () =>
				(await this.lanRouterEnabled()) === enabled &&
				(await this.lanRouterToggleInput.isEnabled()),
		);
	}
}
