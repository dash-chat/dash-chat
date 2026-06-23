import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class ProfilePage extends TestPage {
	back = this.el('profile-back');
	editPhoto = this.el('edit-photo');
	editName = this.el('profile-edit-name');
	editAbout = this.el('profile-edit-about');
	qrLink = this.el('profile-qr-link');

	async ready() {
		await this.editName.waitForExist();
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
