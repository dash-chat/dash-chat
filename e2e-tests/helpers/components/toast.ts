import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class Toast extends TestHelper {
	root = this.el(tid('toast'));

	/** Wait for a toast with the given message text. */
	async expectMessage(message: string): Promise<void> {
		await expect(this.root).toHaveText(message);
	}

	/** Wait for a toast whose text contains `substring`. */
	async expectMessageContaining(substring: string): Promise<void> {
		await expect(this.root).toHaveText(expect.stringContaining(substring));
	}

	/** The most recent toast message, or null if none has fired since the page
	 * loaded. Read from the event ToastManager records rather than the DOM, so a
	 * toast that has already auto-hidden still counts. */
	async lastToastMessage(): Promise<string | null> {
		const message = await this.agent.execute(
			() =>
				(window as Window & { __lastToastEvent?: { message: string } })
					.__lastToastEvent?.message,
		);
		return message ?? null;
	}
}
