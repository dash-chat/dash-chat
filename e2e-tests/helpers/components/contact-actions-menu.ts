import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The contact overflow popover and the block dialog it opens. */
export class ContactActionsMenu extends TestHelper {
	menu = this.el(tid('contact-actions-menu'));
	block = this.el(tid('contact-block'));
	blockConfirm = this.el(tid('block-contact-confirm'));
}
