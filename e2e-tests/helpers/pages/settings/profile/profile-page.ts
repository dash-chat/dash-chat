import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class ProfilePage extends TestHelper {
	back = this.el(tid('profile-back'));
	editPhoto = this.el(tid('edit-photo'));
	editName = this.el(tid('profile-edit-name'));
	editAbout = this.el(tid('profile-edit-about'));
	qrLink = this.el(tid('profile-qr-link'));

	async ready() {
		await this.editName.waitForExist();
	}

	/**
	 * RGB sampled from the avatar shown on this page, once it has an image
	 * loaded. Throws if no avatar image appears.
	 */
	async avatarRgb(): Promise<{ r: number; g: number; b: number }> {
		let rgb: { r: number; g: number; b: number } | null = null;
		await this.agent.waitUntil(
			async () => {
				rgb = await this.agent.execute((sel: string) => {
					const host = document.querySelector(sel);
					const img = host?.shadowRoot?.querySelector('img');
					if (!img?.complete || img.naturalWidth === 0) return null;
					const canvas = document.createElement('canvas');
					canvas.width = 1;
					canvas.height = 1;
					const ctx = canvas.getContext('2d')!;
					ctx.drawImage(img, 0, 0, 1, 1);
					const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
					return { r, g, b };
				}, `${tid('editable-avatar')} wa-avatar`);
				return rgb !== null;
			},
			{ timeoutMsg: 'profile avatar never rendered an image' },
		);
		return rgb!;
	}

	async nameItemContains(name: string): Promise<boolean> {
		return this.agent.execute(
			(sel: string, n: string) =>
				document.querySelector(sel)?.textContent?.includes(n) ?? false,
			tid('profile-edit-name'),
			name,
		);
	}
}
