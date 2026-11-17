import { TestHelper } from '../pages/test-helper';

/** An avatar rendered by `wa-avatar`, wherever in the app it appears. */
export class Avatar extends TestHelper {
	constructor(agent: WebdriverIO.Browser, hostSelector: string) {
		super(agent);
		this.selector = `${hostSelector} wa-avatar`;
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

	/** `initials` is a non-reflecting property of `wa-avatar`, so it is read as one. */
	async initials(): Promise<string | null> {
		return this.agent.execute((sel: string) => {
			const avatar = document.querySelector(sel) as
				| (Element & { initials?: string })
				| null;
			return avatar?.initials || null;
		}, this.selector);
	}

	/** True when `wa-avatar` renders the icon the app slotted in, not an image, initials or its own default glyph. */
	async showsFallbackIcon(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const slot = document
				.querySelector(sel)
				?.shadowRoot?.querySelector<HTMLSlotElement>('slot[name="icon"]');
			return (slot?.assignedElements().length ?? 0) > 0;
		}, this.selector);
	}
}
