import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The checkbox contact picker shared by the new-group members step and the
 * group add-members page. */
export class SelectableContactList extends TestHelper {
	emptyMessage = this.el(tid('selectable-contacts-empty'));

	contactItemSelector(name: string) {
		return `[data-testid="selectable-contact-item"][data-contact-name="${name}"]`;
	}

	contactItem(name: string) {
		return this.agent.$(this.contactItemSelector(name));
	}
}
