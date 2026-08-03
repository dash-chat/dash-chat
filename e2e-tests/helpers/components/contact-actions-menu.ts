import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

/** The contact overflow popover and the block/unblock dialogs it opens. */
export class ContactActionsMenu extends TestHelper {
	menu = this.el(tid('contact-actions-menu'));
	block = this.el(tid('contact-block'));
	unblock = this.el(tid('contact-unblock'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	unblockConfirm = this.el(tid('unblock-contact-confirm'));
}
