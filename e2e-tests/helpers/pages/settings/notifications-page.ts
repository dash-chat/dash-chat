import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class NotificationsPage extends TestHelper {
	back = this.el(tid('notifications-back'));
	toggle = this.el(tid('notifications-toggle'));
	toggleSwitch = this.el(`${tid('notifications-toggle')} label`);
	toggleInput = this.el(`${tid('notifications-toggle')} input`);

	async ready() {
		await this.toggle.waitForExist();
	}

	/** True if the permission settings sheet is open (its markup is in the
	 *  DOM either way, slid out of view while closed). */
	isSettingsSheetOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const inner = document.querySelector(sel);
			if (!inner) return false;
			const root = inner.closest('.k-sheet');
			if (!root) return false;
			return root.classList.contains('-translate-y-full');
		}, tid('permission-settings-sheet'));
	}
}
