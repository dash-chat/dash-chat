import { tid } from '../selectors';

export class Toast {
	constructor(private agent: WebdriverIO.Browser) {}

	root = this.agent.$(tid('toast'));

	/** Wait for a toast with the given message text. */
	async expectMessage(message: string): Promise<void> {
		await expect(this.root).toHaveText(message);
	}
}
