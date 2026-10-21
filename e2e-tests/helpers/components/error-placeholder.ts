import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class ErrorPlaceholder extends TestHelper {
	root = this.el(tid('error-placeholder'));
	retryButton = this.el(tid('error-placeholder-retry'));
}
