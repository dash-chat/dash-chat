import { Avatar } from '../../../components/avatar';
import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class ProfilePage extends TestHelper {
	back = this.el(tid('profile-back'));
	avatar = new Avatar(this.agent, 'editable-avatar');
	editPhoto = this.el(tid('edit-photo'));
	editName = this.el(tid('profile-edit-name'));
	editAbout = this.el(tid('profile-edit-about'));
	qrLink = this.el(tid('profile-qr-link'));

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
