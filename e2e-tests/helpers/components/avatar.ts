import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** An avatar rendered by `wa-avatar`, wherever in the app it appears. */
export class Avatar extends TestHelper {
	constructor(agent: WebdriverIO.Browser, hostTestId: string) {
		super(agent);
		this.selector = `${tid(hostTestId)} wa-avatar`;
	}

	readonly selector: string;

	/**
	 * The image this avatar renders, once it has decoded, as the data URL
	 * `wa-avatar` was handed. Throws if no image appears.
	 */
	async imageSrc(): Promise<string> {
		let src: string | null = null;
		await this.agent.waitUntil(
			async () => {
				src = await this.agent.execute((sel: string) => {
					const img = document
						.querySelector(sel)
						?.shadowRoot?.querySelector('img');
					if (!img?.complete || img.naturalWidth === 0) return null;
					return img.src;
				}, this.selector);
				return src !== null;
			},
			{ timeoutMsg: `${this.selector} never rendered an image` },
		);
		return src!;
	}
}
