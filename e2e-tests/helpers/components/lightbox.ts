import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { MEDIA_SYNC_TIMEOUT } from '../timeouts';

/** The global photo viewer overlay. */
export class Lightbox extends TestHelper {
	root = this.el(tid('lightbox'));
	image = this.el(tid('lightbox-image'));
	close = this.el(tid('lightbox-close'));
	back = this.el(tid('lightbox-back'));
	save = this.el(tid('lightbox-save'));
	prev = this.el(tid('lightbox-prev'));
	next = this.el(tid('lightbox-next'));
	filmstrip = this.el(tid('lightbox-filmstrip'));

	thumb(index: number) {
		return this.agent.$(tid(`lightbox-thumb-${index}`));
	}

	/** The reload/retry affordance inside the thumbnail at `index`, shown when
	 * that thumbnail's image failed to load. */
	thumbRetry(index: number) {
		return this.thumb(index).$(tid('blob-image-retry'));
	}

	/** The reload/retry affordance on the main stage (not a filmstrip thumb),
	 * shown when the displayed photo failed to load. Stage images carry the
	 * `lightbox-image` class; thumbnails do not. */
	stageRetry() {
		return this.agent.$(`.lightbox-image${tid('blob-image-retry')}`);
	}

	/** Simulate a failed load for the thumbnail at `index` (and any image that
	 * shares its alt), surfacing the reload/retry affordance. */
	async forceThumbError(index: number): Promise<void> {
		const alt = await this.thumb(index).$('img').getAttribute('alt');
		if (!alt) throw new Error(`Thumbnail ${index} has no image alt to target`);
		await this.agent.execute((a: string) => {
			window.__test.forceBlobError(a);
		}, alt);
	}

	/** Wait until the filmstrip's thumbnails have decoded — all of them, or
	 * exactly `count` when the caller knows how many to expect. Thumbs render
	 * lazily inside a horizontally scrolling strip, so the ones off-strip never
	 * decode on their own — each poll scrolls the first undecoded one in. */
	async waitForStripLoaded(
		count?: number,
		timeout = MEDIA_SYNC_TIMEOUT,
	): Promise<void> {
		let expected = count ?? 0;
		let loaded = 0;
		await this.agent.waitUntil(
			async () => {
				const strip = await this.agent.execute((stripSel: string) => {
					const imgs = Array.from(
						document.querySelectorAll<HTMLImageElement>(`${stripSel} img`),
					);
					const pending = imgs.filter(
						img => !(img.complete && img.naturalWidth > 0),
					);
					pending[0]?.scrollIntoView({ inline: 'center', block: 'nearest' });
					return {
						// Thumbs that failed render a retry button with no image, so the
						// buttons are what says how many photos the strip is showing.
						total: document.querySelectorAll(`${stripSel} button`).length,
						loaded: imgs.length - pending.length,
					};
				}, tid('lightbox-filmstrip'));
				expected = count ?? strip.total;
				loaded = strip.loaded;
				return expected > 0 && loaded === expected;
			},
			{
				timeout,
				timeoutMsg: `Only ${loaded}/${expected} filmstrip thumbnails loaded`,
			},
		);
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

	/** Close via the header control: the close button, or the back arrow that
	 * replaces it on Android. */
	async clickClose(): Promise<void> {
		if (await this.close.isExisting()) {
			await this.close.click();
			return;
		}
		await this.back.click();
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
