import { switchToWebview } from '../../setup/webview';

/** Android's system dialog asking the user to grant a runtime permission. It
 *  lives outside the webview, so it is driven from the NATIVE_APP context. */
export class PermissionDialog {
	constructor(private agent: WebdriverIO.Browser) {}

	async allow(): Promise<void> {
		await this.agent.switchContext('NATIVE_APP');
		const allowButton = this.agent.$(
			'android=new UiSelector().resourceId("com.android.permissioncontroller:id/permission_allow_button")',
		);
		await allowButton.waitForDisplayed();
		await allowButton.click();
		await switchToWebview(this.agent, 'android');
	}
}
