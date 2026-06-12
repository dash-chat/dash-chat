import { tid } from '../selectors';

/** The global photo viewer overlay. */
export class Lightbox {
	constructor(private agent: WebdriverIO.Browser) {}

	root = this.agent.$(tid('lightbox'));
	image = this.agent.$(tid('lightbox-image'));
	close = this.agent.$(tid('lightbox-close'));
	save = this.agent.$(tid('lightbox-save'));
	prev = this.agent.$(tid('lightbox-prev'));
	next = this.agent.$(tid('lightbox-next'));
	filmstrip = this.agent.$(tid('lightbox-filmstrip'));

	thumb(index: number) {
		return this.agent.$(tid(`lightbox-thumb-${index}`));
	}

	async isOpen(): Promise<boolean> {
		return this.root.isExisting();
	}

	/** The blob URL of the currently displayed photo. */
	async imageSrc(): Promise<string | null> {
		return this.image.getAttribute('src');
	}

	/** Press a key while the lightbox has focus. */
	async pressKey(key: 'Escape' | 'ArrowLeft' | 'ArrowRight'): Promise<void> {
		await this.agent.execute((k: string) => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', {
					key: k,
					bubbles: true,
					cancelable: true,
				}),
			);
		}, key);
	}
}
