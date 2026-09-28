import { switchToWebview } from '../../setup/webview';

/** Android's system dialog asking the user to grant a runtime permission. It
 *  lives outside the webview, so it is driven from the NATIVE_APP context. */
export class PermissionDialog {
	constructor(private agent: WebdriverIO.Browser) {}

	allow(): Promise<void> {
		return this.tap(
			'android=new UiSelector().resourceId("com.android.permissioncontroller:id/permission_allow_button")',
		);
	}

	/** The second refusal shows "Don't allow" under a different id, which also
	 *  stops Android from asking again. */
	deny(): Promise<void> {
		return this.tap(
			'android=new UiSelector().resourceIdMatches("com.android.permissioncontroller:id/permission_deny.*button")',
		);
	}

	private async tap(selector: string): Promise<void> {
		await this.agent.switchContext('NATIVE_APP');
		const button = this.agent.$(selector);
		await button.waitForDisplayed();
		await button.click();
		await switchToWebview(this.agent, 'android');
	}
}
