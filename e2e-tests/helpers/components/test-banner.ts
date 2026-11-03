import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class TestBanner extends TestHelper {
	feedbackButton = this.el(tid('test-banner-feedback'));
}
