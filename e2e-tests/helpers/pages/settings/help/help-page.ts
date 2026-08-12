import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class HelpPage extends TestHelper {
	back = this.el(tid('help-back'));
	contactUsLink = this.el(tid('help-contact-us'));
	versionItem = this.el(tid('help-version'));
	previewFeaturesToggle = this.el(tid('help-preview-features-toggle'));

	async ready() {
		await this.contactUsLink.waitForExist();
	}

	/** Whether preview features are currently switched on. */
	previewFeaturesEnabled(): Promise<boolean> {
		return this.previewFeaturesToggle.$('input').isSelected();
	}

	/** Flip the preview-features switch, waiting until the new state applies.
	 * The checkbox itself is visually hidden, so the label takes the click. */
	async togglePreviewFeatures(): Promise<void> {
		const before = await this.previewFeaturesEnabled();
		await this.previewFeaturesToggle.$('label').click();
		await this.agent.waitUntil(
			async () => (await this.previewFeaturesEnabled()) !== before,
			{ timeoutMsg: 'Preview-features toggle did not change state' },
		);
	}
}
