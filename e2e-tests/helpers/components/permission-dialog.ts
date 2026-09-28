import type { Agent } from '../../setup/setup-agents';
import { switchToWebview } from '../../setup/webview';

/** Android's system dialog asking the user to grant a runtime permission. It
 *  lives outside the webview, so it is driven from the NATIVE_APP context. */
export class PermissionDialog {
	constructor(private agent: Agent) {}

	async allow(): Promise<void> {
		await this.agent.switchContext('NATIVE_APP');
		try {
			const allowButton = this.agent.$(
				'android=new UiSelector().resourceIdMatches(".*permission_allow.*button")',
			);
			await allowButton.waitForDisplayed({
				timeoutMsg: 'no Allow button on a system permission dialog',
			});
			await allowButton.click();
		} finally {
			await switchToWebview(this.agent, this.agent.platform);
		}
	}
}
