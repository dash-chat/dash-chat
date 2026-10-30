import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class TestBanner extends TestHelper {
	banner = this.el(tid('test-banner'));
	feedbackButton = this.el(tid('test-banner-feedback-btn'));
}
