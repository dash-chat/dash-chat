import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The global photo viewer overlay. */
export class Lightbox extends TestHelper {
	root = this.el(tid('lightbox'));
	image = this.el(tid('lightbox-image'));
	close = this.el(tid('lightbox-close'));
	save = this.el(tid('lightbox-save'));
	prev = this.el(tid('lightbox-prev'));
	next = this.el(tid('lightbox-next'));
	filmstrip = this.el(tid('lightbox-filmstrip'));

	thumb(index: number) {
		return this.agent.$(tid(`lightbox-thumb-${index}`));
	}

	/** Index of the currently active photo (based on the selected filmstrip thumb). */
	async activeIndex(): Promise<number> {
		const thumbs = await this.agent.$$(tid('lightbox-filmstrip') + ' button');
		const resolved = await thumbs;
		for (let i = 0; i < (await resolved.length); i++) {
			const cls = await resolved[i].getAttribute('class');
			if (cls?.includes('selected')) return i;
		}
		return -1;
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
