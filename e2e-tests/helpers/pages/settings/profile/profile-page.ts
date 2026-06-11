import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class ProfilePage extends TestPage {
	back = this.agent.$(tid('profile-back'));
	editPhoto = this.agent.$(tid('edit-photo'));
	editName = this.agent.$(tid('profile-edit-name'));
	editAbout = this.agent.$(tid('profile-edit-about'));
	qrLink = this.agent.$(tid('profile-qr-link'));

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
