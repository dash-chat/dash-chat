import { tid } from '../selectors';

export class Toast {
	constructor(private agent: WebdriverIO.Browser) {}

	root = this.agent.$(tid('toast'));

	/** Wait for a toast with the given message text. */
	async expectMessage(message: string): Promise<void> {
		await expect(this.root).toHaveText(message);
	}

	/** Wait for a toast whose text contains `substring`. */
	async expectMessageContaining(substring: string): Promise<void> {
		await expect(this.root).toHaveText(expect.stringContaining(substring));
	}
}
