import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The checkbox contact picker shared by the new-group members step and the
 * group add-members page. */
export class SelectableContactList extends TestHelper {
	emptyMessage = this.el(tid('selectable-contacts-empty'));

	contactItem(name: string) {
		return this.agent.$(
			`[data-testid="selectable-contact-item"][data-contact-name="${name}"]`,
		);
	}
}
