import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The contact overflow popover and the block/report dialogs it opens. */
export class ContactActionsMenu extends TestHelper {
	menu = this.el(tid('contact-actions-menu'));
	block = this.el(tid('contact-block'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	report = this.el(tid('contact-report'));
	reportConfirm = this.el(tid('report-contact-confirm'));
}
