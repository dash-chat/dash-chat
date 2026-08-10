import { ContactActionsMenu } from '../../components/contact-actions-menu';
import { simulateLongpress } from '../../long-press';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class NewMessagePage extends TestHelper {
	back = this.el(tid('new-message-back'));
	search = this.el(tid('new-message-search'));
	addContact = this.el(`${tid('new-message-add-contact')} a`);
	newGroup = this.el(`${tid('new-message-new-group')} a`);
	contactList = this.el(tid('new-message-contact-list'));
	emptyMessage = this.el(tid('new-message-contacts-empty'));
	contactActionsMenu = new ContactActionsMenu(this.agent);

	contactItemSelector(name: string) {
		return `[data-testid="new-message-contact-item"][data-contact-name="${name}"]`;
	}

	contactItem(name: string) {
		return this.agent.$(this.contactItemSelector(name));
	}

	async ready() {
		await this.addContact.waitForExist();
	}

	/** Open a contact's actions menu with the gesture its platform offers: a
	 * long-press on the row on mobile, the overflow button on desktop. The
	 * button is JS-clicked because it is only revealed on hover. */
	async openContactMenu(name: string) {
		const row = this.contactItem(name);
		await row.waitForExist();

		if (await this.isMobileBuild()) {
			await simulateLongpress(
				this.agent,
				`${this.contactItemSelector(name)} a`,
			);
		} else {
			await this.agent.execute(
				(sel: string) => {
					(document.querySelector(sel) as HTMLElement | null)?.click();
				},
				`${this.contactItemSelector(name)} ${tid('contact-menu-button')}`,
			);
		}

		await this.contactActionsMenu.menu.waitForDisplayed();
	}
}
