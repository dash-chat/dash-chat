import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class AppearancePage extends TestHelper {
	back = this.el(tid('appearance-back'));
	light = this.el(tid('appearance-light'));
	dark = this.el(tid('appearance-dark'));
	system = this.el(tid('appearance-system'));

	async ready() {
		await this.light.waitForExist();
	}

	/** The colour scheme the app currently has applied. */
	getColorScheme(): Promise<'light' | 'dark'> {
		return this.agent.execute(() =>
			document.documentElement.classList.contains('dark') ? 'dark' : 'light',
		);
	}
}
