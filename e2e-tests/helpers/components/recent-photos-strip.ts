import { TINY_PNG_DATA_URL } from '../images';
import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/**
 * The composer's recent-photos strip (mobile media panel). The native photo
 * library is unavailable in the test harness, so {@link injectPhotos} feeds it
 * fake photos through the `window.__test.recentPhotos` seam.
 */
export class RecentPhotosStrip extends TestHelper {
	strip = this.el(tid('message-input-recent-photos'));
	allowButton = this.el(tid('message-input-recent-photos-allow'));

	tile(index: number) {
		return this.agent.$(tid(`message-input-recent-photo-${index}`));
	}

	/** Make the strip show `count` granted, selectable photos. Call before the
	 * media panel is opened so the strip reads them on mount. */
	async injectPhotos(count: number): Promise<void> {
		await this.agent.execute(
			(n: number, dataUrl: string) => {
				window.__test.recentPhotos = {
					permission: 'granted',
					photos: Array.from({ length: n }, (_, i) => ({
						id: `recent-${i}`,
						name: `recent-${i}.png`,
						mimeType: 'image/png',
						dataUrl,
					})),
				};
			},
			count,
			TINY_PNG_DATA_URL,
		);
	}

	/** Make the strip show the "Allow access" prompt instead of photos. */
	async injectPrompt(): Promise<void> {
		await this.agent.execute(() => {
			window.__test.recentPhotos = { permission: 'prompt', photos: [] };
		});
	}

	async clearInjected(): Promise<void> {
		await this.agent.execute(() => {
			window.__test.recentPhotos = undefined;
		});
	}

	async isVisible(): Promise<boolean> {
		return this.strip.isExisting();
	}
}
